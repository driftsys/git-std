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
    npx markdownlint-cli '**/*.md' --ignore node_modules

# Audit dependencies
audit:
    cargo audit

# Run all checks (test + lint + audit)
check: test lint audit

# Assemble + check
build: assemble check

# Validate commits on branch and build — run before PR
verify:
    git std check --range main..HEAD
    just build

# Format Rust and Markdown
fmt:
    cargo fmt
    dprint fmt

# Generate and open rustdoc
doc:
    cargo doc --open

# Build and serve mdbook documentation
book:
    mdbook serve

[private]
pre-publish: check
    @echo "==> Dry-run publish all crates..."
    cargo publish --dry-run -p standard-commit
    cargo publish --dry-run -p standard-changelog
    cargo publish --dry-run -p standard-version
    cargo publish --dry-run -p standard-githooks
    cargo publish --dry-run -p git-std

# Publish all crates to crates.io (dependency order)
publish: pre-publish
    @echo "==> 1/5 standard-commit"
    cargo publish -p standard-commit
    sleep 5
    @echo "==> 2/5 standard-changelog"
    cargo publish -p standard-changelog
    @echo "==> 3/5 standard-version"
    cargo publish -p standard-version
    @echo "==> 4/5 standard-githooks"
    cargo publish -p standard-githooks
    sleep 5
    @echo "==> 5/5 git-std"
    cargo publish -p git-std
    @echo ""
    @echo "==> All crates published."

# Remove build artifacts
clean:
    cargo clean
