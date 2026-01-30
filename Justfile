# List available recipes
default:
    @just --list

# Run the full CI suite
ci: fmt clippy test deny

# Run all checks
check: fmt clippy test

# Run tests
test:
    cargo nextest run --workspace --all-features

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
    .github/ensure-cargo-deny.sh
    cargo deny check

# Clean build artifacts
clean:
    cargo clean

# Start the devnet with interactive DKG
devnet:
    cd docker && just devnet

# Stop the devnet
devnet-down:
    cd docker && just down

# Reset devnet (clears all state, requires fresh DKG)
devnet-reset:
    cd docker && just reset

# View devnet logs
devnet-logs:
    cd docker && just logs

# View devnet status
devnet-status:
    cd docker && just status

# Live devnet monitoring dashboard
devnet-stats:
    cd docker && just stats

# Build docker images
docker-build:
    cd docker && just build
