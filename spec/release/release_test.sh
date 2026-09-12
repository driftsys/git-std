#!/usr/bin/env bash

repo_root="$(git rev-parse --show-toplevel)"
package_script="$repo_root/scripts/release/package.sh"
verify_script="$repo_root/scripts/release/verify.sh"
fixture_dir="$repo_root/spec/release/fixtures"

setup() {
    release_tmp="$(mktemp -d)"
    mkdir -p "$release_tmp/staging" "$release_tmp/output"
    printf '#!/usr/bin/env sh\necho git-std\n' > "$release_tmp/git-std"
    chmod +x "$release_tmp/git-std"
    printf '.TH GIT-STD 1\n' > "$release_tmp/staging/git-std.1"
}

teardown() {
    rm -rf "$release_tmp"
}

digest_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

validate_workflow_shape() {
    ruby -ryaml -e '
        document = YAML.safe_load(File.read(ARGV.fetch(0)), aliases: true)
        jobs = document.fetch("jobs")
        tag_only = "github.event_name == '\''push'\'' && startsWith(github.ref, '\''refs/tags/v'\'')"
        abort "release job is not tag-only" unless jobs.fetch("release").fetch("if") == tag_only
        abort "publish job is not tag-only" unless jobs.fetch("publish").fetch("if") == tag_only
        build = jobs.fetch("build")
        targets = build.fetch("strategy").fetch("matrix").fetch("include").map { |row| row.fetch("target") }
        expected = %w[x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-msvc]
        abort "wrong release target matrix" unless targets == expected
        steps = build.fetch("steps")
        package_run = steps.find { |step| step["name"] == "Package archive and checksum" }.fetch("run")
        package_lines = package_run.lines.map(&:strip).reject(&:empty?)
        expected_package_lines = [
          "scripts/release/package.sh \\",
          "\"${{ matrix.target }}\" \\",
          "\"target/${{ matrix.target }}/release/git-std${{ matrix.ext || " + (39.chr * 2) + " }}\" \\",
          "man-staging \\",
          "release-assets",
        ]
        abort "package invocation is not exact" unless package_lines == expected_package_lines
        hosted_verify = steps.find { |step| step["name"] == "Verify local and hosted release evidence" }.fetch("run")
        hosted_lines = hosted_verify.lines.map(&:strip).reject(&:empty?)
        expected_hosted_lines = [
          "scripts/release/verify.sh \\",
          "\"${{ steps.release-metadata.outputs.archive }}\" \\",
          "\"${{ steps.release-metadata.outputs.version }}\" \\",
          "--hosted",
        ]
        abort "hosted verification is not wired" unless hosted_lines == expected_hosted_lines
        upload_paths = steps.find { |step| step["name"] == "Upload artifacts" }.fetch("with").fetch("path").lines.map(&:strip).reject(&:empty?)
        release_files = jobs.fetch("release").fetch("steps").find { |step| step["name"] == "Create release" }.fetch("with").fetch("files").lines.map(&:strip).reject(&:empty?)
        upload_prefix = "release-assets/git-std-${{ matrix.target }}"
        release_prefix = "git-std-*"
        suffixes = %w[.tar.gz .tar.gz.sha256 .tar.gz.sigstore.json .tar.gz.spdx.json .tar.gz.provenance.sigstore.json]
        abort "wrong build upload entries" unless upload_paths == suffixes.map { |suffix| upload_prefix + suffix }
        abort "wrong release upload entries" unless release_files == suffixes.map { |suffix| release_prefix + suffix }
    ' "$1"
}

create_bundle() {
    local target="${1:-x86_64-unknown-linux-musl}"
    local executable="$release_tmp/git-std"
    if [[ "$target" == *windows* ]]; then
        executable="$release_tmp/git-std.exe"
        cp "$release_tmp/git-std" "$executable"
    fi
    bash "$package_script" "$target" "$executable" "$release_tmp/staging" "$release_tmp/output"
    archive="$release_tmp/output/git-std-$target.tar.gz"
    local digest
    digest="$(digest_file "$archive")"
    jq -n \
        --arg name "$(basename "$archive")" \
        --arg version "v1.2.3" \
        --arg digest "$digest" \
        '{spdxVersion:"SPDX-2.3",name:("git-std-release-"+$version),packages:[{name:$name,versionInfo:$version,checksums:[{algorithm:"SHA256",checksumValue:$digest}]}]}' \
        > "$archive.spdx.json"
    printf '{"mediaType":"application/vnd.dev.sigstore.bundle+json;version=0.3","testVerification":"valid"}\n' \
        > "$archive.sigstore.json"
    printf '{"mediaType":"application/vnd.dev.sigstore.bundle+json;version=0.3","testVerification":"valid"}\n' \
        > "$archive.provenance.sigstore.json"
}

install_hosted_verifiers() {
    fake_bin="$release_tmp/fake-bin"
    verifier_log="$release_tmp/verifier.log"
    mkdir -p "$fake_bin"
    cp "$fixture_dir/cosign" "$fixture_dir/gh" "$fake_bin/"
    chmod +x "$fake_bin/cosign" "$fake_bin/gh"
}

test_package_names_all_supported_targets_and_layout() {
    local target executable expected
    for target in \
        x86_64-unknown-linux-musl \
        aarch64-unknown-linux-musl \
        x86_64-apple-darwin \
        aarch64-apple-darwin \
        x86_64-pc-windows-msvc
    do
        executable="$release_tmp/git-std"
        expected="git-std"
        if [[ "$target" == *windows* ]]; then
            executable="$release_tmp/git-std.exe"
            cp "$release_tmp/git-std" "$executable"
            expected="git-std.exe"
        fi
        assert "bash '$package_script' '$target' '$executable' '$release_tmp/staging' '$release_tmp/output'"
        assert "test -f '$release_tmp/output/git-std-$target.tar.gz'"
        assert "test -f '$release_tmp/output/git-std-$target.tar.gz.sha256'"
        assert "tar -tzf '$release_tmp/output/git-std-$target.tar.gz' | sed 's#^./##' | grep -qx '$expected'"
        assert "tar -tzf '$release_tmp/output/git-std-$target.tar.gz' | sed 's#^./##' | grep -qx 'git-std.1'"
    done
}

test_verify_accepts_complete_local_bundle() {
    create_bundle
    assert "bash '$verify_script' '$archive' v1.2.3"
}

test_verify_rejects_tampered_archive() {
    create_bundle
    printf 'tampered' >> "$archive"
    assert_fails "bash '$verify_script' '$archive' v1.2.3"
}

test_verify_rejects_wrong_sbom_version_or_digest() {
    create_bundle
    jq '.packages[0].versionInfo = "v9.9.9"' "$archive.spdx.json" > "$archive.spdx.tmp"
    mv "$archive.spdx.tmp" "$archive.spdx.json"
    assert_fails "bash '$verify_script' '$archive' v1.2.3"

    create_bundle
    jq '.packages[0].checksums[0].checksumValue = "deadbeef"' "$archive.spdx.json" > "$archive.spdx.tmp"
    mv "$archive.spdx.tmp" "$archive.spdx.json"
    assert_fails "bash '$verify_script' '$archive' v1.2.3"
}

test_verify_rejects_missing_signature_bundle() {
    create_bundle
    rm "$archive.sigstore.json"
    assert_fails "bash '$verify_script' '$archive' v1.2.3"
}

test_verify_rejects_missing_provenance_bundle() {
    create_bundle
    rm "$archive.provenance.sigstore.json"
    assert_fails "bash '$verify_script' '$archive' v1.2.3"
}

test_hosted_verify_binds_archive_to_signature_and_provenance() {
    create_bundle
    install_hosted_verifiers

    assert "EXPECTED_ARCHIVE='$archive' VERIFIER_LOG='$verifier_log' PATH='$fake_bin:$PATH' bash '$verify_script' '$archive' v1.2.3 --hosted"
    assert_equals $'cosign\ngh' "$(cat "$verifier_log")"
}

test_hosted_verify_rejects_signature_verifier_failure() {
    create_bundle
    install_hosted_verifiers
    jq '.testVerification = "tampered"' "$archive.sigstore.json" > "$archive.sigstore.tmp"
    mv "$archive.sigstore.tmp" "$archive.sigstore.json"

    assert_fails "EXPECTED_ARCHIVE='$archive' VERIFIER_LOG='$verifier_log' PATH='$fake_bin:$PATH' bash '$verify_script' '$archive' v1.2.3 --hosted"
    assert_equals "cosign" "$(cat "$verifier_log")"
}

test_hosted_verify_rejects_provenance_verifier_failure() {
    create_bundle
    install_hosted_verifiers
    jq '.testVerification = "tampered"' "$archive.provenance.sigstore.json" > "$archive.provenance.tmp"
    mv "$archive.provenance.tmp" "$archive.provenance.sigstore.json"

    assert_fails "EXPECTED_ARCHIVE='$archive' VERIFIER_LOG='$verifier_log' PATH='$fake_bin:$PATH' bash '$verify_script' '$archive' v1.2.3 --hosted"
    assert_equals $'cosign\ngh' "$(cat "$verifier_log")"
}

test_workflow_declares_nonpublishing_dispatch_and_metadata_assets() {
    local workflow="$repo_root/.github/workflows/release.yml"
    assert "validate_workflow_shape '$workflow'"
    if command -v actionlint >/dev/null 2>&1; then
        assert "actionlint '$workflow'"
    fi
    assert "grep -q 'workflow_dispatch:' '$workflow'"
    assert "grep -q 'attest-build-provenance@v3' '$workflow'"
    assert "grep -q '\\.sigstore.json' '$workflow'"
    assert "grep -q '\\.spdx.json' '$workflow'"
    assert "grep -q '\\.provenance.sigstore.json' '$workflow'"
}

test_just_test_exports_invoking_worktree_as_source_root() {
    local fake_bin="$release_tmp/fake-bin"
    local observed="$release_tmp/cargo-source-root"
    mkdir -p "$fake_bin"
    printf '#!/usr/bin/env bash\nprintf "%%s" "${CARGO_RUSTC_CURRENT_DIR-}" > "$OBSERVED"\n' \
        > "$fake_bin/cargo"
    chmod +x "$fake_bin/cargo"

    assert "cd '$repo_root' && env -u CARGO_RUSTC_CURRENT_DIR OBSERVED='$observed' PATH='$fake_bin:$PATH' just test >/dev/null"
    assert_equals "$repo_root" "$(cat "$observed")"
}
