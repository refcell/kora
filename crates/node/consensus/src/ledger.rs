//! Ledger view aggregate for state management.

use std::collections::BTreeSet;

use alloy_primitives::B256;
use kora_domain::{StateRoot, Tx};
use kora_qmdb::ChangeSet;
use kora_traits::StateDb;

use crate::{ConsensusError, Digest, Mempool, SeedTracker, Snapshot, SnapshotStore, TxId};

/// Aggregate that owns all state management components.
///
/// LedgerView coordinates access to the mempool, snapshots, seeds, and
/// persistent state database. It provides high-level operations for
/// block proposal and verification.
#[derive(Debug)]
pub struct LedgerView<S, M, SS, ST> {
    /// Persistent state database.
    state: S,
    /// Pending transaction pool.
    mempool: M,
    /// Execution snapshots keyed by digest.
    snapshots: SS,
    /// VRF seeds for prevrandao computation.
    seeds: ST,
}

impl<S, M, SS, ST> LedgerView<S, M, SS, ST>
where
    S: StateDb,
    M: Mempool,
    SS: SnapshotStore<S>,
    ST: SeedTracker,
{
    /// Create a new ledger view.
    pub const fn new(state: S, mempool: M, snapshots: SS, seeds: ST) -> Self {
        Self { state, mempool, snapshots, seeds }
    }

    /// Get a reference to the state database.
    pub const fn state(&self) -> &S {
        &self.state
    }

    /// Get a mutable reference to the state database.
    pub const fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }

    /// Get a reference to the mempool.
    pub const fn mempool(&self) -> &M {
        &self.mempool
    }

    /// Get a mutable reference to the mempool.
    pub const fn mempool_mut(&mut self) -> &mut M {
        &mut self.mempool
    }

    /// Get a reference to the snapshot store.
    pub const fn snapshots(&self) -> &SS {
        &self.snapshots
    }

    /// Get a mutable reference to the snapshot store.
    pub const fn snapshots_mut(&mut self) -> &mut SS {
        &mut self.snapshots
    }

    /// Get a reference to the seed tracker.
    pub const fn seeds(&self) -> &ST {
        &self.seeds
    }

    /// Get a mutable reference to the seed tracker.
    pub const fn seeds_mut(&mut self) -> &mut ST {
        &mut self.seeds
    }

    /// Build a batch of transactions for a new proposal.
    ///
    /// Collects transaction IDs from all unpersisted ancestor snapshots
    /// (starting from `parent`) to exclude already-included transactions,
    /// then returns up to `max_txs` transactions from the mempool.
    ///
    /// # Arguments
    ///
    /// * `parent` - The parent block digest (or `None` for genesis).
    /// * `max_txs` - Maximum number of transactions to return.
    /// # Returns
    ///
    /// A vector of transactions suitable for inclusion in a new block.
    pub fn build_proposal_txs(&self, parent: Option<Digest>, max_txs: usize) -> Vec<Tx> {
        let excluded = self.collect_ancestor_tx_ids(parent);
        self.mempool.build(max_txs, &excluded)
    }

    /// Collect transaction IDs from unpersisted ancestor blocks.
    ///
    /// Currently returns an empty set since `Snapshot<S>` does not contain
    /// transaction data. Transaction deduplication relies on the mempool's
    /// prune mechanism after finalization.
    fn collect_ancestor_tx_ids(&self, _parent: Option<Digest>) -> BTreeSet<TxId> {
        let mut excluded = BTreeSet::new();
        let mut current = _parent;

        while let Some(digest) = current {
            if self.snapshots.is_persisted(&digest) {
                break;
            }

            let Some(snapshot) = self.snapshots.get(&digest) else {
                break;
            };
            excluded.extend(snapshot.tx_ids.iter().copied());
            current = snapshot.parent;
        }

        excluded
    }

    /// Get the snapshot for a parent digest.
    ///
    /// Returns `None` if the parent is genesis (no parent).
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot is not found for a non-genesis parent.
    pub fn get_parent_snapshot(
        &self,
        parent: Option<Digest>,
    ) -> Result<Option<Snapshot<S>>, ConsensusError> {
        match parent {
            Some(digest) => {
                let snapshot =
                    self.snapshots.get(&digest).ok_or(ConsensusError::SnapshotNotFound(digest))?;
                Ok(Some(snapshot))
            }
            None => Ok(None),
        }
    }

    /// Insert a new snapshot into the cache.
    pub fn insert_snapshot(&self, digest: Digest, snapshot: Snapshot<S>) {
        self.snapshots.insert(digest, snapshot);
    }

    /// Get the prevrandao seed for a given digest.
    ///
    /// # Errors
    ///
    /// Returns an error if the seed is not found.
    pub fn get_seed(&self, digest: &Digest) -> Result<B256, ConsensusError> {
        self.seeds.get(digest).ok_or(ConsensusError::SnapshotNotFound(*digest))
    }

    /// Insert a seed for a digest.
    pub fn insert_seed(&self, digest: Digest, seed: B256) {
        self.seeds.insert(digest, seed);
    }

    /// Persist a finalized snapshot and all its unpersisted ancestors.
    ///
    /// This commits the merged changes from all unpersisted ancestors up to
    /// and including the given digest to the state database.
    ///
    /// # Arguments
    ///
    /// * `digest` - The digest of the finalized block.
    ///
    /// # Errors
    ///
    /// Returns an error if a snapshot in the chain is missing or if the
    /// state database commit fails.
    pub async fn persist_snapshot(&self, digest: Digest) -> Result<StateRoot, ConsensusError> {
        // Get the chain of unpersisted digests and merged changes
        let (chain, merged_changes) = self.snapshots.changes_for_persist(digest)?;

        if chain.is_empty() {
            // Already persisted, return current state root
            return Ok(StateRoot(self.state.state_root().await?));
        }

        // Commit the merged changes to the state database
        let state_root = StateRoot(self.state.commit(merged_changes).await?);

        // Mark all digests in the chain as persisted
        self.snapshots.mark_persisted(&chain);

        Ok(state_root)
    }

    /// Get merged changes from the last persisted ancestor up to and including
    /// the parent, then merge with new changes.
    ///
    /// This is useful for computing speculative state roots before a block
    /// is finalized.
    pub fn merged_changes(
        &self,
        parent: Digest,
        new_changes: ChangeSet,
    ) -> Result<ChangeSet, ConsensusError> {
        self.snapshots.merged_changes(parent, new_changes)
    }

    /// Check if a digest has been persisted.
    pub fn is_persisted(&self, digest: &Digest) -> bool {
        self.snapshots.is_persisted(digest)
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::U256;
    use kora_domain::{StateRoot, Tx};
    use kora_qmdb::ChangeSet;

    use super::*;
    use crate::components::{InMemoryMempool, InMemorySeedTracker, InMemorySnapshotStore};

    // Mock StateDb for testing
    #[derive(Clone, Debug)]
    struct MockStateDb {
        root: B256,
    }

    impl MockStateDb {
        fn new() -> Self {
            Self { root: B256::ZERO }
        }
    }

    impl kora_traits::StateDbRead for MockStateDb {
        async fn nonce(
            &self,
            _address: &alloy_primitives::Address,
        ) -> Result<u64, kora_traits::StateDbError> {
            Ok(0)
        }

        async fn balance(
            &self,
            _address: &alloy_primitives::Address,
        ) -> Result<U256, kora_traits::StateDbError> {
            Ok(U256::ZERO)
        }

        async fn code_hash(
            &self,
            _address: &alloy_primitives::Address,
        ) -> Result<B256, kora_traits::StateDbError> {
            Ok(B256::ZERO)
        }

        async fn code(
            &self,
            _code_hash: &B256,
        ) -> Result<alloy_primitives::Bytes, kora_traits::StateDbError> {
            Ok(alloy_primitives::Bytes::new())
        }

        async fn storage(
            &self,
            _address: &alloy_primitives::Address,
            _slot: &U256,
        ) -> Result<U256, kora_traits::StateDbError> {
            Ok(U256::ZERO)
        }
    }

    impl kora_traits::StateDbWrite for MockStateDb {
        async fn commit(&self, _changes: ChangeSet) -> Result<B256, kora_traits::StateDbError> {
            Ok(B256::repeat_byte(0xCC))
        }

        async fn compute_root(
            &self,
            _changes: &ChangeSet,
        ) -> Result<B256, kora_traits::StateDbError> {
            Ok(B256::ZERO)
        }

        fn merge_changes(&self, mut older: ChangeSet, newer: ChangeSet) -> ChangeSet {
            older.merge(newer);
            older
        }
    }

    impl kora_traits::StateDb for MockStateDb {
        async fn state_root(&self) -> Result<B256, kora_traits::StateDbError> {
            Ok(self.root)
        }
    }

    type TestLedgerView = LedgerView<
        MockStateDb,
        InMemoryMempool,
        InMemorySnapshotStore<MockStateDb>,
        InMemorySeedTracker,
    >;

    fn create_test_ledger() -> TestLedgerView {
        let state = MockStateDb::new();
        let mempool = InMemoryMempool::new();
        let snapshots = InMemorySnapshotStore::new();
        let seeds = InMemorySeedTracker::empty();
        LedgerView::new(state, mempool, snapshots, seeds)
    }

    fn digest(byte: u8) -> Digest {
        Digest::from([byte; 32])
    }

    #[test]
    fn ledger_view_new() {
        let ledger = create_test_ledger();
        assert!(ledger.mempool().is_empty());
    }

    #[test]
    fn ledger_view_accessors() {
        let mut ledger = create_test_ledger();

        // Test state accessors
        let _ = ledger.state();
        let _ = ledger.state_mut();

        // Test mempool accessors
        let _ = ledger.mempool();
        let _ = ledger.mempool_mut();

        // Test snapshot accessors
        let _ = ledger.snapshots();
        let _ = ledger.snapshots_mut();

        // Test seed accessors
        let _ = ledger.seeds();
        let _ = ledger.seeds_mut();
    }

    #[test]
    fn ledger_view_build_proposal_txs() {
        let ledger = create_test_ledger();

        // Add some transactions
        ledger.mempool().insert(Tx::new(vec![1, 2, 3].into()));
        ledger.mempool().insert(Tx::new(vec![4, 5, 6].into()));

        // Build proposal without parent
        let txs = ledger.build_proposal_txs(None, 10);
        assert_eq!(txs.len(), 2);
    }

    #[test]
    fn ledger_view_get_parent_snapshot_genesis() {
        let ledger = create_test_ledger();

        // Genesis has no parent
        let result = ledger.get_parent_snapshot(None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn ledger_view_get_parent_snapshot_missing() {
        let ledger = create_test_ledger();

        let digest = digest(0x01);
        let result = ledger.get_parent_snapshot(Some(digest));
        assert!(result.is_err());

        match result {
            Err(ConsensusError::SnapshotNotFound(d)) => assert_eq!(d, digest),
            _ => panic!("expected SnapshotNotFound error"),
        }
    }

    #[test]
    fn ledger_view_get_parent_snapshot_found() {
        let ledger = create_test_ledger();

        let digest = digest(0x01);
        let snapshot = Snapshot::new(
            None,
            MockStateDb::new(),
            StateRoot(B256::ZERO),
            ChangeSet::new(),
            BTreeSet::new(),
        );

        ledger.insert_snapshot(digest, snapshot);

        let result = ledger.get_parent_snapshot(Some(digest));
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn ledger_view_insert_snapshot() {
        let ledger = create_test_ledger();

        let digest = digest(0x01);
        assert!(ledger.snapshots().get(&digest).is_none());

        let snapshot = Snapshot::new(
            None,
            MockStateDb::new(),
            StateRoot(B256::ZERO),
            ChangeSet::new(),
            BTreeSet::new(),
        );
        ledger.insert_snapshot(digest, snapshot);

        assert!(ledger.snapshots().get(&digest).is_some());
    }

    #[test]
    fn ledger_view_seed_operations() {
        let ledger = create_test_ledger();

        let digest = digest(0x01);
        let seed = B256::repeat_byte(0x02);

        // Initially missing
        assert!(ledger.get_seed(&digest).is_err());

        // Insert and retrieve
        ledger.insert_seed(digest, seed);
        assert_eq!(ledger.get_seed(&digest).unwrap(), seed);
    }

    #[tokio::test]
    async fn ledger_view_persist_snapshot() {
        let ledger = create_test_ledger();

        let digest = digest(0x01);
        let snapshot = Snapshot::new(
            None,
            MockStateDb::new(),
            StateRoot(B256::ZERO),
            ChangeSet::new(),
            BTreeSet::new(),
        );

        ledger.insert_snapshot(digest, snapshot);
        assert!(!ledger.is_persisted(&digest));

        let result = ledger.persist_snapshot(digest).await;
        assert!(result.is_ok());

        assert!(ledger.is_persisted(&digest));
    }

    #[tokio::test]
    async fn ledger_view_persist_snapshot_chain() {
        let ledger = create_test_ledger();

        let digest1 = digest(0x01);
        let digest2 = digest(0x02);
        let digest3 = digest(0x03);

        // Create a chain: digest1 -> digest2 -> digest3
        let snap1 = Snapshot::new(
            None,
            MockStateDb::new(),
            StateRoot(B256::ZERO),
            ChangeSet::new(),
            BTreeSet::new(),
        );
        let snap2 = Snapshot::new(
            Some(digest1),
            MockStateDb::new(),
            StateRoot(B256::ZERO),
            ChangeSet::new(),
            BTreeSet::new(),
        );
        let snap3 = Snapshot::new(
            Some(digest2),
            MockStateDb::new(),
            StateRoot(B256::ZERO),
            ChangeSet::new(),
            BTreeSet::new(),
        );

        ledger.insert_snapshot(digest1, snap1);
        ledger.insert_snapshot(digest2, snap2);
        ledger.insert_snapshot(digest3, snap3);

        // Persist the chain by persisting the tip
        let result = ledger.persist_snapshot(digest3).await;
        assert!(result.is_ok());

        // All should be persisted
        assert!(ledger.is_persisted(&digest1));
        assert!(ledger.is_persisted(&digest2));
        assert!(ledger.is_persisted(&digest3));
    }

    #[test]
    fn ledger_view_merged_changes() {
        let ledger = create_test_ledger();

        let digest = digest(0x01);
        let snapshot = Snapshot::new(
            None,
            MockStateDb::new(),
            StateRoot(B256::ZERO),
            ChangeSet::new(),
            BTreeSet::new(),
        );

        ledger.insert_snapshot(digest, snapshot);

        let result = ledger.merged_changes(digest, ChangeSet::new());
        assert!(result.is_ok());
    }
}
