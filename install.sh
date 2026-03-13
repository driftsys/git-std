#!/usr/bin/env bash
set -euo pipefail

REPO="driftsys/git-std"
INSTALL_DIR="${GIT_STD_INSTALL_DIR:-$HOME/.local/bin}"

die() { printf 'error: %s\n' "$1" >&2; exit 1; }

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64)  echo "x86_64-unknown-linux-musl" ;;
        aarch64) echo "aarch64-unknown-linux-musl" ;;
        *)       die "unsupported architecture: $arch" ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64)  echo "x86_64-apple-darwin" ;;
        arm64)   echo "aarch64-apple-darwin" ;;
        *)       die "unsupported architecture: $arch" ;;
      esac
      ;;
    *)
      die "unsupported OS: $os (use WSL on Windows)"
      ;;
  esac
}

main() {
  local target version download_url tmp_dir

  target="$(detect_target)"
  printf 'detected target: %s\n' "$target"

  # Get latest release tag
  version="$(curl -sSf "https://api.github.com/repos/$REPO/releases/latest" \
    | grep '"tag_name"' | head -1 | cut -d'"' -f4)"
  [ -n "$version" ] || die "could not determine latest release"
  printf 'latest version: %s\n' "$version"

  download_url="https://github.com/$REPO/releases/download/$version/git-std-$target"
  printf 'downloading %s\n' "$download_url"

  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "$tmp_dir"' EXIT

  curl -sSfL "$download_url" -o "$tmp_dir/git-std" \
    || die "download failed — check that the release exists for $target"

  mkdir -p "$INSTALL_DIR"
  mv "$tmp_dir/git-std" "$INSTALL_DIR/git-std"
  chmod +x "$INSTALL_DIR/git-std"

  printf 'installed git-std to %s/git-std\n' "$INSTALL_DIR"

  # Verify
  if command -v git-std >/dev/null 2>&1; then
    printf 'version: %s\n' "$(git-std --version)"
  else
    printf 'note: %s is not in your PATH — add it to use "git std"\n' "$INSTALL_DIR"
  fi
}

main "$@"
