# kora-executor

Block execution abstractions and REVM-based implementation for Kora.

This crate provides:
- `BlockExecutor` trait for executing transactions against state
- `ExecutionOutcome` with receipts and state changes
- `RevmExecutor` implementing block execution via REVM
