# kora-reporters

Consensus reporters for Kora nodes.

This crate provides reusable `commonware_consensus::Reporter` implementations used by
Kora node applications, including:

- `SeedReporter` - captures simplex activity seeds and hashes them for later proposals
- `FinalizedReporter` - replays finalized blocks, validates roots, and persists snapshots

These reporters are designed to work with `kora-ledger` and can be used in both the
example REVM chain and production nodes.
