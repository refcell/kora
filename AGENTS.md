# Agent Guidelines for Kora

## Build & Test Commands

```sh
# Build in release mode
cargo build --release

# Build all targets
cargo build --all-targets

# Run tests
cargo nextest run --workspace --all-features

# Run the full CI suite (format, clippy, test, deny)
just ci

# Check formatting
cargo +nightly fmt --all -- --check

# Fix formatting
cargo +nightly fmt --all

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings

# Run cargo deny
just deny
```

## Development Commands

```sh
# Start the devnet with interactive DKG
just devnet

# Start the devnet with trusted dealer DKG (fast, for local dev)
just trusted-devnet

# Stop the devnet
just devnet-down

# View devnet logs
just devnet-logs
```

## Project Structure

- `bin/kora` - Main binary
- `crates/node/` - Node components (consensus, executor, rpc, etc.)
- `crates/network/` - Networking (transport, marshal)
- `crates/storage/` - Storage (qmdb, handlers, indexer)
- `crates/utilities/` - Shared utilities (cli, crypto, sys)
- `crates/e2e/` - End-to-end tests

## Code Conventions

- Use `#![doc = include_str!("../README.md")]` in lib.rs to embed README as docs
- Every crate should have a README.md with badges
- Use workspace lints and dependencies
- Follow the type-state pattern for builders
- Use `thiserror` for error types
- Prefer `const fn` where possible

## Workspace Lints

The workspace enforces:
- `missing-docs` (warn)
- `missing-debug-implementations` (warn)
- `unreachable-pub` (warn)
- `unused-must-use` (deny)
- `rust-2018-idioms` (deny)

## Testing

- Unit tests use `#[cfg(test)]` modules
- E2E tests are in `crates/e2e`
- Use `rstest` for parameterized tests
