# `kora-primitives`

<a href="https://github.com/anthropics/kora/actions/workflows/ci.yml"><img src="https://github.com/anthropics/kora/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://github.com/anthropics/kora/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg" alt="License"></a>

Core primitive types for the Kora execution client.

## Overview

This crate provides foundational types used throughout the Kora codebase:

- **`Address`**: Ethereum address type (re-exported from `alloy-primitives`).
- **`B256`**: 256-bit hash type (re-exported from `alloy-primitives`).
- **`U256`**: 256-bit unsigned integer type (re-exported from `alloy-primitives`).

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
kora-primitives = { path = "crates/primitives" }
```

Use the types:

```rust,ignore
use kora_primitives::{Address, B256, U256};

let address = Address::ZERO;
let hash = B256::ZERO;
let value = U256::from(1000u64);
```

## License

[MIT License](https://github.com/anthropics/kora/blob/main/LICENSE)
