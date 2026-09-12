#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 || $# -gt 3 ]]; then
    echo "usage: verify.sh <archive> <version> [--hosted]" >&2
    exit 2
fi

archive="$1"
version="$2"
mode="${3:-}"
[[ -z "$mode" || "$mode" == "--hosted" ]] || { echo "error: unknown mode: $mode" >&2; exit 2; }

fail() { echo "error: $1" >&2; exit 1; }
digest_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

[[ -f "$archive" ]] || fail "archive not found: $archive"
archive_name="$(basename "$archive")"
target="${archive_name#git-std-}"
target="${target%.tar.gz}"
case "$target" in
    x86_64-unknown-linux-musl | aarch64-unknown-linux-musl | \
        x86_64-apple-darwin | aarch64-apple-darwin | x86_64-pc-windows-msvc) ;;
    *) fail "unsupported archive name: $archive_name" ;;
esac

checksum="$archive.sha256"
sbom="$archive.spdx.json"
signature="$archive.sigstore.json"
provenance="$archive.provenance.sigstore.json"
for companion in "$checksum" "$sbom" "$signature" "$provenance"; do
    [[ -f "$companion" ]] || fail "required companion missing: $(basename "$companion")"
done

expected_digest="$(awk 'NR == 1 { print $1 }' "$checksum")"
actual_digest="$(digest_file "$archive")"
[[ "$expected_digest" == "$actual_digest" ]] || fail "archive checksum mismatch"
grep -Eq "^[0-9a-f]{64}  ${archive_name}$" "$checksum" || fail "checksum subject name mismatch"

expected_binary="git-std"
[[ "$target" == "x86_64-pc-windows-msvc" ]] && expected_binary="git-std.exe"
contents="$(tar -tzf "$archive" | sed 's#^\./##' | sed '/^$/d')"
grep -qx "$expected_binary" <<<"$contents" || fail "archive executable layout mismatch"
grep -qx 'git-std.1' <<<"$contents" || fail "archive is missing git-std.1"
if grep -Evq "^(${expected_binary//./\\.}|git-std[^/]*\\.1)$" <<<"$contents"; then
    fail "archive contains an unexpected path"
fi

jq -e \
    --arg name "$archive_name" \
    --arg version "$version" \
    --arg digest "$actual_digest" \
    '.spdxVersion == "SPDX-2.3" and
     (.name | contains($version)) and
     any(.packages[]?;
       .name == $name and .versionInfo == $version and
       any(.checksums[]?; .algorithm == "SHA256" and .checksumValue == $digest))' \
    "$sbom" >/dev/null || fail "SBOM does not identify the archive version and digest"
jq -e 'type == "object" and (.mediaType | type == "string")' "$signature" >/dev/null \
    || fail "invalid Sigstore signature bundle"
jq -e 'type == "object" and (.mediaType | type == "string")' "$provenance" >/dev/null \
    || fail "invalid provenance bundle"

if [[ "$mode" == "--hosted" ]]; then
    command -v cosign >/dev/null || fail "cosign is required for hosted verification"
    command -v gh >/dev/null || fail "gh is required for hosted verification"
    cosign verify-blob \
        --bundle "$signature" \
        --certificate-identity-regexp '^https://github.com/driftsys/git-std/.github/workflows/release.yml@refs/' \
        --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
        "$archive"
    gh attestation verify "$archive" \
        --repo driftsys/git-std \
        --bundle "$provenance" \
        --signer-workflow driftsys/git-std/.github/workflows/release.yml
fi

echo "verified $archive_name ($version, sha256:$actual_digest)"
