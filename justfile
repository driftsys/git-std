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

# Remove build artifacts
clean:
    cargo clean
