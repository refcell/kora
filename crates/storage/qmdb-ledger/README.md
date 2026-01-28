# kora-qmdb-ledger

QMDB-backed ledger adapter for Kora.

This crate bundles the QMDB backend, handlers, and state traits into a
single helper that can initialize genesis allocations, compute roots,
and commit changes.

## Key Types

- `QmdbLedger` - QMDB-backed ledger service
- `QmdbConfig` - configuration for the QMDB backend
- `QmdbState` - state handle used by executors
- `QmdbChangeSet` - change set type for QMDB writes

## Usage

```rust,ignore
use kora_qmdb_ledger::{QmdbConfig, QmdbLedger};

let ledger = QmdbLedger::init(context, QmdbConfig::new(prefix, pool), allocations).await?;
let state = ledger.state();
```
