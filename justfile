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
    npx markdownlint-cli '**/*.md' --ignore node_modules --fix

# Generate and open rustdoc
doc:
    cargo doc --open

# Build and serve mdbook documentation
book:
    mdbook serve

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

# Remove build artifacts
clean:
    cargo clean
