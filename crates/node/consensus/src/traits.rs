//! Core trait abstractions for consensus components.

use std::collections::BTreeSet;

use alloy_primitives::B256;
use kora_qmdb::ChangeSet;
use kora_traits::StateDb;

use crate::ConsensusError;

/// Transaction identifier type.
pub type TxId = B256;

/// Consensus digest type.
pub type Digest = B256;

/// A snapshot of execution state at a specific block.
#[derive(Clone, Debug)]
pub struct Snapshot<S> {
    /// Parent block digest.
    pub parent: Option<Digest>,
    /// State database at this point.
    pub state: S,
    /// Computed state root.
    pub state_root: B256,
    /// Pending state changes not yet persisted.
    pub changes: ChangeSet,
}

impl<S> Snapshot<S> {
    /// Create a new snapshot.
    pub const fn new(
        parent: Option<Digest>,
        state: S,
        state_root: B256,
        changes: ChangeSet,
    ) -> Self {
        Self { parent, state, state_root, changes }
    }
}

/// Mempool provides access to pending transactions for block building.
///
/// Implementations may use different ordering strategies (FIFO, priority, etc).
pub trait Mempool: Clone + Send + Sync + 'static {
    /// Transaction type stored in the mempool.
    type Tx: Clone + Send + Sync + 'static;

    /// Insert a transaction into the mempool.
    ///
    /// Returns `true` if the transaction was newly inserted.
    fn insert(&self, tx: Self::Tx) -> bool;

    /// Build a batch of transactions for inclusion in a block.
    ///
    /// `excluded` contains transaction IDs already included in pending ancestor blocks.
    /// `max_txs` limits the number of transactions returned.
    fn build(&self, max_txs: usize, excluded: &BTreeSet<TxId>) -> Vec<Self::Tx>;

    /// Remove finalized transactions from the mempool.
    fn prune(&self, tx_ids: &[TxId]);

    /// Get the current number of pending transactions.
    fn len(&self) -> usize;

    /// Check if the mempool is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Manages execution snapshots keyed by block digest.
///
/// Snapshots allow replaying execution from any ancestor and computing
/// speculative state roots before finalization.
pub trait SnapshotStore<S: StateDb>: Clone + Send + Sync + 'static {
    /// Get a snapshot by digest.
    fn get(&self, digest: &Digest) -> Option<Snapshot<S>>;

    /// Insert a new snapshot.
    fn insert(&self, digest: Digest, snapshot: Snapshot<S>);

    /// Check if a digest has been persisted to the underlying state db.
    fn is_persisted(&self, digest: &Digest) -> bool;

    /// Mark a chain of digests as persisted.
    fn mark_persisted(&self, digests: &[Digest]);

    /// Get merged changes from the last persisted ancestor up to and including
    /// the given parent, then merge with the provided new changes.
    fn merged_changes(
        &self,
        parent: Digest,
        new_changes: ChangeSet,
    ) -> Result<ChangeSet, ConsensusError>;

    /// Get the chain of unpersisted digests and merged changes for persistence.
    fn changes_for_persist(
        &self,
        digest: Digest,
    ) -> Result<(Vec<Digest>, ChangeSet), ConsensusError>;
}

/// Tracks VRF seeds for prevrandao computation.
///
/// Seeds are derived from threshold VRF signatures during consensus and
/// used to populate the `prevrandao` field in subsequent blocks.
pub trait SeedTracker: Clone + Send + Sync + 'static {
    /// Get the seed for a given digest.
    fn get(&self, digest: &Digest) -> Option<B256>;

    /// Insert a seed for a digest.
    fn insert(&self, digest: Digest, seed: B256);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_new() {
        let snapshot: Snapshot<()> = Snapshot::new(None, (), B256::ZERO, ChangeSet::new());
        assert!(snapshot.parent.is_none());
        assert_eq!(snapshot.state_root, B256::ZERO);
    }
}
