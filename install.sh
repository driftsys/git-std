#!/usr/bin/env bash
set -euo pipefail

REPO="driftsys/git-std"
INSTALL_DIR="${GIT_STD_INSTALL_DIR:-$HOME/.local/bin}"
tmp_dir=""

die() { printf 'error: %s\n' "$1" >&2; exit 1; }

detect_shell() {
  case "$(basename "${SHELL:-}")" in
    bash) echo "bash" ;;
    zsh)  echo "zsh"  ;;
    fish) echo "fish" ;;
    *)    echo ""     ;;
  esac
}

install_completions() {
  local shell rc_file snippet
  shell="$(detect_shell)"
  case "$shell" in
    bash) rc_file="$HOME/.bashrc"; snippet="eval \"\$(git-std --completions bash)\"" ;;
    zsh)  rc_file="$HOME/.zshrc";  snippet="eval \"\$(git-std --completions zsh)\""  ;;
    fish)
      mkdir -p "$HOME/.config/fish/conf.d"
      rc_file="$HOME/.config/fish/conf.d/git-std.fish"
      snippet='git-std --completions fish | source'
      ;;
    *)
      printf 'note: unknown shell %s — add completions manually\n' "${SHELL:-}" >&2
      return
      ;;
  esac

  # Don't create a missing RC file (fish conf.d/ dir already created above).
  if [ "$shell" != "fish" ] && [ ! -f "$rc_file" ]; then
    printf 'note: %s not found — add completions manually\n' "$rc_file" >&2
    return
  fi

  if grep -q 'git-std --completions' "$rc_file" 2>/dev/null; then
    printf 'completions already configured in %s\n' "$rc_file"
    return
  elif grep -q 'git-std completions' "$rc_file" 2>/dev/null; then
    sed -i.bak 's/git-std completions /git-std --completions /g' "$rc_file" && rm -f "${rc_file}.bak"
    printf 'completions migrated in %s\n' "$rc_file"
    return
  fi

  printf '\n# git-std completions\n%s\n' "$snippet" >> "$rc_file"
  printf 'completions installed to %s\n' "$rc_file"
  printf 'note: restart your shell or run: source %s\n' "$rc_file"
}

sha256_check() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$1"
  else
    shasum -a 256 -c "$1"
  fi
}

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
  local target version download_url base

  target="$(detect_target)"
  printf 'detected target: %s\n' "$target"

  # Get latest release tag
  version="$(curl -sSf "https://api.github.com/repos/$REPO/releases/latest" \
    | grep '"tag_name"' | head -1 | cut -d'"' -f4)"
  [ -n "$version" ] || die "could not determine latest release"
  printf 'latest version: %s\n' "$version"

  local base="git-std-$target"
  download_url="https://github.com/$REPO/releases/download/$version/$base.tar.gz"
  printf 'downloading %s\n' "$download_url"

  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "${tmp_dir:-}"' EXIT

  curl -sSfL "$download_url" -o "$tmp_dir/$base.tar.gz" \
    || die "download failed — check that the release exists for $target"
  curl -sSfL "$download_url.sha256" -o "$tmp_dir/$base.tar.gz.sha256" \
    || die "checksum download failed"

  # Verify checksum
  (cd "$tmp_dir" && sha256_check "$base.tar.gz.sha256") \
    || die "checksum verification failed"

  tar -xzf "$tmp_dir/$base.tar.gz" -C "$tmp_dir"

  mkdir -p "$INSTALL_DIR"
  mv "$tmp_dir/git-std" "$INSTALL_DIR/git-std"
  chmod +x "$INSTALL_DIR/git-std"

  printf 'installed git-std to %s/git-std\n' "$INSTALL_DIR"

  # Install man pages if present in the tarball.
  local man_dir="${GIT_STD_MAN_DIR:-$HOME/.local/share/man/man1}"
  if ls "$tmp_dir"/git-std*.1 >/dev/null 2>&1; then
    mkdir -p "$man_dir"
    cp "$tmp_dir"/git-std*.1 "$man_dir/"
    printf 'installed man pages to %s\n' "$man_dir"
    printf "hint: if 'man git-std' doesn't work, add to your shell profile:\n"
    printf "      export MANPATH=\"\$HOME/.local/share/man:\${MANPATH:-}\"\n"
  fi

  # Install shell completions.
  install_completions

  # Verify
  if command -v git-std >/dev/null 2>&1; then
    printf 'version: %s\n' "$(git-std --version)"
  else
    printf 'note: %s is not in your PATH — add it to use "git std"\n' "$INSTALL_DIR"
  fi
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then main "$@"; fi
