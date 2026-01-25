# List available recipes
default:
    @just --list

# Run the full CI suite
ci: fmt clippy test deny

# Run all checks
check: fmt clippy test

# Run tests
test:
    cargo nextest run --all-features

# Build in release mode
build:
    cargo build --release

# Build all targets
build-all:
    cargo build --all-targets

# Check formatting
fmt:
    cargo +nightly fmt --all -- --check

# Fix formatting
fmt-fix:
    cargo +nightly fmt --all

# Run clippy
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Run cargo deny
deny:
    cargo deny check

# Clean build artifacts
clean:
    cargo clean
