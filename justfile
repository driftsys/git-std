# Compile
assemble:
    cargo build

# Run tests
test:
    cargo test

# Lint and format check
lint:
    cargo clippy -- -D warnings
    cargo fmt -- --check
    dprint check
    npx markdownlint-cli '**/*.md' --ignore node_modules --ignore skills

# Audit dependencies
audit:
    cargo audit

# Run all checks (test + lint + audit)
check: test lint audit

# Assemble + check
build: assemble check

# Validate commits on branch and build — run before PR
verify:
    git std lint --range main..HEAD
    just build

# Format Rust and Markdown
fmt:
    cargo fmt
    dprint fmt
    npx markdownlint-cli '**/*.md' --ignore node_modules --ignore skills --fix

# Generate man pages to target/man/
man:
    cargo build -p git-std
    mkdir -p target/man
    find target/ -path '*/build/git-std-*/out/man/*.1' -exec cp {} target/man/ \;
    @echo "man pages written to target/man/"

# Generate and open rustdoc
doc:
    cargo doc --open

# Build and serve mdbook documentation
book:
    mdbook serve

# Bump version, update changelog, commit, and tag
release:
    git std bump

# Publish all crates to crates.io (dependency order)
# Skips separate dry-run — cargo publish already verifies
# each package before uploading.
publish: check
    @echo "==> 1/5 standard-commit"
    cargo publish -p standard-commit
    @echo "==> 2/5 standard-changelog"
    cargo publish -p standard-changelog
    @echo "==> 3/5 standard-version"
    cargo publish -p standard-version
    @echo "==> 4/5 standard-githooks"
    cargo publish -p standard-githooks
    @echo "==> 5/5 git-std"
    cargo publish -p git-std
    @echo ""
    @echo "==> All crates published."

# Build release binary, man pages, and shell completions, then install to ~/.local/bin
install:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release
    cp target/release/git-std ~/.local/bin/

    just man
    mkdir -p ~/.local/share/man/man1
    cp target/man/*.1 ~/.local/share/man/man1/
    printf "hint: if 'man git-std' doesn't work, add to your shell profile:\n"
    printf "      export MANPATH=\"\$HOME/.local/share/man:\${MANPATH:-}\"\n"

    shell="$(basename "${SHELL:-}")"
    case "$shell" in
      bash) rc="$HOME/.bashrc"; s="eval \"\$(git-std --completions bash)\"" ;;
      zsh)  rc="$HOME/.zshrc";  s="eval \"\$(git-std --completions zsh)\""  ;;
      fish)
        mkdir -p "$HOME/.config/fish/conf.d"
        rc="$HOME/.config/fish/conf.d/git-std.fish"
        s='git-std --completions fish | source'
        ;;
      *)
        printf 'note: add completions manually for %s\n' "${SHELL:-}" >&2
        exit 0
        ;;
    esac
    if [ "$shell" != "fish" ] && [ ! -f "$rc" ]; then
      printf 'note: %s not found — add completions manually\n' "$rc" >&2
      exit 0
    fi
    if grep -q 'git-std --completions' "$rc" 2>/dev/null; then
      printf 'completions already configured in %s\n' "$rc"
    elif grep -q 'git-std completions' "$rc" 2>/dev/null; then
      sed -i.bak 's/git-std completions /git-std --completions /g' "$rc" && rm -f "${rc}.bak"
      printf 'completions migrated in %s\n' "$rc"
    else
      printf '\n# git-std completions\n%s\n' "$s" >> "$rc"
      printf 'completions installed to %s\n' "$rc"
    fi

# Remove build artifacts
clean:
    cargo clean
