# kora-txpool

Production-ready transaction pool for kora-node with validation, nonce ordering, and fee prioritization.

## Features

- **Per-sender nonce-ordered queues**: Transactions are organized by sender with proper nonce ordering
- **Pending vs queued separation**: Executable transactions (pending) are separated from future nonce transactions (queued)
- **Fee-based ordering**: Transactions with higher effective gas prices are prioritized
- **Transaction validation**: Signature recovery, chain ID validation, intrinsic gas calculation, balance checks
- **Configurable limits**: Max pool size, per-sender limits, max transaction size, minimum gas price

## Usage

```rust,ignore
use kora_txpool::{TransactionPool, PoolConfig, TransactionValidator};

// Create a pool with default configuration
let config = PoolConfig::default();
let pool = TransactionPool::new(config);

// Create a validator with chain ID, state access, and config
let validator = TransactionValidator::new(chain_id, state_db, config);

// Validate and add a transaction
let validated = validator.validate(raw_tx).await?;
let ordered = validated.into_ordered(timestamp);
pool.add(ordered)?;

// Get pending transactions for block building
let pending = pool.pending(100);
```
