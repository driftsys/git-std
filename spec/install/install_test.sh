#!/usr/bin/env bash
# Tests for install.sh pure functions.
#
# Run with: bash tools/bash_unit tests/install_test.sh
#
# Sourcing install.sh loads the helper functions without executing main()
# because of the [[ "${BASH_SOURCE[0]}" == "${0}" ]] guard at the bottom.

# shellcheck source=../install.sh
. "$(git rev-parse --show-toplevel)/install.sh"

# ── detect_target ─────────────────────────────────────────────────────────────

test_detect_linux_x86_64() {
    uname() { case "$1" in -s) echo "Linux" ;; -m) echo "x86_64" ;; esac }
    export -f uname
    assert_equals "x86_64-unknown-linux-musl" "$(detect_target)"
}

test_detect_linux_aarch64() {
    uname() { case "$1" in -s) echo "Linux" ;; -m) echo "aarch64" ;; esac }
    export -f uname
    assert_equals "aarch64-unknown-linux-musl" "$(detect_target)"
}

test_detect_macos_x86_64() {
    uname() { case "$1" in -s) echo "Darwin" ;; -m) echo "x86_64" ;; esac }
    export -f uname
    assert_equals "x86_64-apple-darwin" "$(detect_target)"
}

test_detect_macos_arm64() {
    uname() { case "$1" in -s) echo "Darwin" ;; -m) echo "arm64" ;; esac }
    export -f uname
    assert_equals "aarch64-apple-darwin" "$(detect_target)"
}

test_detect_unsupported_os_fails() {
    uname() { case "$1" in -s) echo "Windows_NT" ;; -m) echo "x86_64" ;; esac }
    export -f uname
    assert_fails "detect_target"
}

test_detect_unsupported_arch_fails() {
    uname() { case "$1" in -s) echo "Linux" ;; -m) echo "riscv64" ;; esac }
    export -f uname
    assert_fails "detect_target"
}

# ── URL construction ───────────────────────────────────────────────────────────
# Verify the download URL pattern matches actual release asset names.
# Regression guard for #187 (missing .tar.gz extension).

test_url_has_tar_gz_extension() {
    local target="x86_64-unknown-linux-musl"
    local version="v1.0.0"
    local url="https://github.com/driftsys/git-std/releases/download/$version/git-std-$target.tar.gz"
    assert_matches "\.tar\.gz$" "$url"
}

test_url_checksum_has_sha256_suffix() {
    local target="aarch64-apple-darwin"
    local version="v1.2.3"
    local url="https://github.com/driftsys/git-std/releases/download/$version/git-std-$target.tar.gz.sha256"
    assert_matches "\.tar\.gz\.sha256$" "$url"
}

test_url_contains_target_triple() {
    local target="x86_64-apple-darwin"
    local version="v0.11.10"
    local url="https://github.com/driftsys/git-std/releases/download/$version/git-std-$target.tar.gz"
    assert_matches "$target" "$url"
}

test_url_contains_version_tag() {
    local target="x86_64-unknown-linux-musl"
    local version="v0.11.10"
    local url="https://github.com/driftsys/git-std/releases/download/$version/git-std-$target.tar.gz"
    assert_matches "$version" "$url"
}

# ── sha256_check ──────────────────────────────────────────────────────────────

test_sha256_check_valid_file_passes() {
    local tmp
    tmp="$(mktemp -d)"
    echo "test archive content" > "$tmp/archive.tar.gz"
    (
        cd "$tmp"
        if command -v sha256sum >/dev/null 2>&1; then
            sha256sum archive.tar.gz > archive.tar.gz.sha256
        else
            shasum -a 256 archive.tar.gz > archive.tar.gz.sha256
        fi
        sha256_check archive.tar.gz.sha256
    )
    rm -rf "$tmp"
}

test_sha256_check_tampered_file_fails() {
    local tmp
    tmp="$(mktemp -d)"
    echo "original content" > "$tmp/archive.tar.gz"
    (
        cd "$tmp"
        if command -v sha256sum >/dev/null 2>&1; then
            sha256sum archive.tar.gz > archive.tar.gz.sha256
        else
            shasum -a 256 archive.tar.gz > archive.tar.gz.sha256
        fi
    )
    echo "tampered content" > "$tmp/archive.tar.gz"
    assert_fails "(cd '$tmp' && sha256_check archive.tar.gz.sha256)"
    rm -rf "$tmp"
}
