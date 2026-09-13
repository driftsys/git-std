#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 4 ]]; then
    echo "usage: package.sh <target> <executable> <man-staging-dir> <output-dir>" >&2
    exit 2
fi

target="$1"
executable="$2"
man_staging="$3"
output_dir="$4"

case "$target" in
    x86_64-unknown-linux-musl | aarch64-unknown-linux-musl | \
        x86_64-apple-darwin | aarch64-apple-darwin | x86_64-pc-windows-msvc) ;;
    *)
        echo "error: unsupported release target: $target" >&2
        exit 2
        ;;
esac

[[ -f "$executable" ]] || { echo "error: executable not found: $executable" >&2; exit 2; }
[[ -d "$man_staging" ]] || { echo "error: man staging directory not found: $man_staging" >&2; exit 2; }
[[ -f "$man_staging/git-std.1" ]] || { echo "error: git-std.1 not found in $man_staging" >&2; exit 2; }

mkdir -p "$output_dir"
package_staging="$(mktemp -d "$output_dir/.git-std-package.XXXXXX")"
trap 'rm -rf "$package_staging"' EXIT

binary_name="git-std"
if [[ "$target" == "x86_64-pc-windows-msvc" ]]; then
    binary_name="git-std.exe"
fi
cp "$executable" "$package_staging/$binary_name"
find "$man_staging" -maxdepth 1 -type f -name 'git-std*.1' -exec cp {} "$package_staging/" \;

archive_name="git-std-$target.tar.gz"
archive="$output_dir/$archive_name"
COPYFILE_DISABLE=1 tar -czf "$archive" -C "$package_staging" .

if command -v sha256sum >/dev/null 2>&1; then
    digest="$(sha256sum "$archive" | cut -d' ' -f1)"
else
    digest="$(shasum -a 256 "$archive" | cut -d' ' -f1)"
fi
printf '%s  %s\n' "$digest" "$archive_name" > "$archive.sha256"
