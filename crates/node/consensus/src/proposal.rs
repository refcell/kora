//! Block proposal building logic.

use std::collections::BTreeSet;

use alloy_consensus::Header;
use alloy_primitives::{Address, B256, Bytes};
use commonware_cryptography::Committable as _;
use kora_domain::{Block, StateRoot, Tx};
use kora_executor::{BlockContext, BlockExecutor};
use kora_traits::StateDb;

use crate::{ConsensusError, Digest, Mempool, Snapshot, SnapshotStore, TxId};

fn block_context(height: u64, prevrandao: B256) -> BlockContext {
    let header = Header {
        number: height,
        timestamp: height,
        gas_limit: kora_config::DEFAULT_GAS_LIMIT,
        beneficiary: Address::ZERO,
        base_fee_per_gas: Some(0),
        ..Default::default()
    };
    BlockContext::new(header, B256::ZERO, prevrandao)
}

/// Builder for constructing block proposals.
///
/// ProposalBuilder coordinates gathering transactions from the mempool,
/// executing them against parent state, and constructing a complete block.
#[derive(Debug)]
pub struct ProposalBuilder<S, M, SS, E> {
    /// State database for execution.
    state: S,
    /// Transaction mempool.
    mempool: M,
    /// Snapshot store for parent state lookup.
    snapshots: SS,
    /// Block executor.
    executor: E,
    /// Maximum transactions per block.
    max_txs: usize,
}

impl<S, M, SS, E> ProposalBuilder<S, M, SS, E>
where
    S: StateDb,
    M: Mempool,
    SS: SnapshotStore<S>,
    E: BlockExecutor<S, Tx = Bytes>,
{
    /// Default maximum transactions per block.
    pub const DEFAULT_MAX_TXS: usize = 1000;

    /// Create a new proposal builder.
    ///
    /// # Arguments
    ///
    /// * `state` - State database for execution lookups.
    /// * `mempool` - Transaction mempool to pull transactions from.
    /// * `snapshots` - Snapshot store for parent state lookup.
    /// * `executor` - Block executor for transaction execution.
    pub const fn new(state: S, mempool: M, snapshots: SS, executor: E) -> Self {
        Self { state, mempool, snapshots, executor, max_txs: Self::DEFAULT_MAX_TXS }
    }

    /// Set the maximum number of transactions per block.
    ///
    /// Defaults to [`Self::DEFAULT_MAX_TXS`].
    #[must_use]
    pub const fn with_max_txs(mut self, max_txs: usize) -> Self {
        self.max_txs = max_txs;
        self
    }

    /// Build a block proposal from the given parent block.
    ///
    /// This method:
    /// 1. Retrieves the parent snapshot from the snapshot store.
    /// 2. Builds a transaction batch from the mempool, excluding the parent's txs.
    /// 3. Executes the batch against the parent state.
    /// 4. Computes the new state root from the execution outcome.
    /// 5. Constructs and returns the new block and its snapshot.
    pub fn build_proposal(
        &self,
        parent: &Block,
        prevrandao: B256,
    ) -> Result<(Block, Snapshot<S>), ConsensusError> {
        let parent_digest = parent.commitment();
        let parent_snapshot = self
            .snapshots
            .get(&parent_digest)
            .ok_or(ConsensusError::SnapshotNotFound(parent_digest))?;

        let excluded = self.collect_pending_tx_ids(parent_digest)?;
        let txs = self.mempool.build(self.max_txs, &excluded);

        let height = parent.height + 1;
        let context = block_context(height, prevrandao);
        let txs_bytes: Vec<Bytes> = txs.iter().map(|tx| tx.bytes.clone()).collect();
        let outcome = self
            .executor
            .execute(&parent_snapshot.state, &context, &txs_bytes)
            .map_err(|e| ConsensusError::Execution(e.to_string()))?;

        let merged_changes =
            self.snapshots.merged_changes(parent_digest, outcome.changes.clone())?;
        let state_root = futures::executor::block_on(self.state.compute_root(&merged_changes))
            .map_err(ConsensusError::StateDb)?;
        let state_root = StateRoot(state_root);

        let block = Block { parent: parent.id(), height, prevrandao, state_root, txs };
        let tx_ids = self.tx_ids_from_block(&block);
        let snapshot = Snapshot::new(
            Some(parent_digest),
            parent_snapshot.state,
            state_root,
            outcome.changes,
            tx_ids,
        );

        Ok((block, snapshot))
    }

    /// Async variant of [`Self::build_proposal`] that awaits state root computation.
    pub async fn build_proposal_async(
        &self,
        parent: &Block,
        prevrandao: B256,
    ) -> Result<(Block, Snapshot<S>), ConsensusError> {
        let parent_digest = parent.commitment();
        let parent_snapshot = self
            .snapshots
            .get(&parent_digest)
            .ok_or(ConsensusError::SnapshotNotFound(parent_digest))?;

        let excluded = self.collect_pending_tx_ids(parent_digest)?;
        let txs = self.mempool.build(self.max_txs, &excluded);

        let height = parent.height + 1;
        let context = block_context(height, prevrandao);
        let txs_bytes: Vec<Bytes> = txs.iter().map(|tx| tx.bytes.clone()).collect();
        let outcome = self
            .executor
            .execute(&parent_snapshot.state, &context, &txs_bytes)
            .map_err(|e| ConsensusError::Execution(e.to_string()))?;

        let merged_changes =
            self.snapshots.merged_changes(parent_digest, outcome.changes.clone())?;
        let state_root =
            self.state.compute_root(&merged_changes).await.map_err(ConsensusError::StateDb)?;
        let state_root = StateRoot(state_root);

        let block = Block { parent: parent.id(), height, prevrandao, state_root, txs };
        let tx_ids = self.tx_ids_from_block(&block);
        let snapshot = Snapshot::new(
            Some(parent_digest),
            parent_snapshot.state,
            state_root,
            outcome.changes,
            tx_ids,
        );

        Ok((block, snapshot))
    }

    fn tx_ids_from_block(&self, block: &Block) -> BTreeSet<TxId> {
        block.txs.iter().map(Tx::id).collect()
    }

    fn collect_pending_tx_ids(&self, from: Digest) -> Result<BTreeSet<TxId>, ConsensusError> {
        let mut excluded = BTreeSet::new();
        let mut current = Some(from);

        while let Some(digest) = current {
            if self.snapshots.is_persisted(&digest) {
                break;
            }

            let snapshot =
                self.snapshots.get(&digest).ok_or(ConsensusError::SnapshotNotFound(digest))?;
            excluded.extend(snapshot.tx_ids.iter().copied());
            current = snapshot.parent;
        }

        Ok(excluded)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, RwLock},
    };

    use alloy_primitives::{Address, Bytes, U256};
    use kora_executor::ExecutionOutcome;
    use kora_qmdb::ChangeSet;

    use super::*;

    // Mock implementations for testing

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
        async fn nonce(&self, _address: &Address) -> Result<u64, kora_traits::StateDbError> {
            Ok(0)
        }

        async fn balance(&self, _address: &Address) -> Result<U256, kora_traits::StateDbError> {
            Ok(U256::ZERO)
        }

        async fn code_hash(&self, _address: &Address) -> Result<B256, kora_traits::StateDbError> {
            Ok(B256::ZERO)
        }

        async fn code(&self, _code_hash: &B256) -> Result<Bytes, kora_traits::StateDbError> {
            Ok(Bytes::new())
        }

        async fn storage(
            &self,
            _address: &Address,
            _slot: &U256,
        ) -> Result<U256, kora_traits::StateDbError> {
            Ok(U256::ZERO)
        }
    }

    impl kora_traits::StateDbWrite for MockStateDb {
        async fn commit(&self, _changes: ChangeSet) -> Result<B256, kora_traits::StateDbError> {
            Ok(B256::repeat_byte(0x42))
        }

        async fn compute_root(
            &self,
            _changes: &ChangeSet,
        ) -> Result<B256, kora_traits::StateDbError> {
            Ok(B256::repeat_byte(0x42))
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

    #[derive(Clone)]
    struct MockMempool {
        txs: Arc<RwLock<BTreeMap<TxId, Tx>>>,
    }

    impl MockMempool {
        fn new() -> Self {
            Self { txs: Arc::new(RwLock::new(BTreeMap::new())) }
        }

        fn add(&self, tx: Tx) {
            let id = tx.id();
            self.txs.write().unwrap().insert(id, tx);
        }
    }

    impl Mempool for MockMempool {
        fn insert(&self, tx: Tx) -> bool {
            let id = tx.id();
            self.txs.write().unwrap().insert(id, tx).is_none()
        }

        fn build(&self, max_txs: usize, excluded: &BTreeSet<TxId>) -> Vec<Tx> {
            self.txs
                .read()
                .unwrap()
                .iter()
                .filter(|(id, _)| !excluded.contains(id))
                .take(max_txs)
                .map(|(_, tx)| tx.clone())
                .collect()
        }

        fn prune(&self, tx_ids: &[TxId]) {
            let mut txs = self.txs.write().unwrap();
            for id in tx_ids {
                txs.remove(id);
            }
        }

        fn len(&self) -> usize {
            self.txs.read().unwrap().len()
        }
    }

    #[derive(Clone)]
    struct MockSnapshotStore {
        snapshots: Arc<RwLock<BTreeMap<Digest, Snapshot<MockStateDb>>>>,
        persisted: Arc<RwLock<BTreeSet<Digest>>>,
    }

    impl MockSnapshotStore {
        fn new() -> Self {
            Self {
                snapshots: Arc::new(RwLock::new(BTreeMap::new())),
                persisted: Arc::new(RwLock::new(BTreeSet::new())),
            }
        }
    }

    impl SnapshotStore<MockStateDb> for MockSnapshotStore {
        fn get(&self, digest: &Digest) -> Option<Snapshot<MockStateDb>> {
            self.snapshots.read().unwrap().get(digest).cloned()
        }

        fn insert(&self, digest: Digest, snapshot: Snapshot<MockStateDb>) {
            self.snapshots.write().unwrap().insert(digest, snapshot);
        }

        fn is_persisted(&self, digest: &Digest) -> bool {
            self.persisted.read().unwrap().contains(digest)
        }

        fn mark_persisted(&self, digests: &[Digest]) {
            let mut persisted = self.persisted.write().unwrap();
            for digest in digests {
                persisted.insert(*digest);
            }
        }

        fn merged_changes(
            &self,
            _parent: Digest,
            new_changes: ChangeSet,
        ) -> Result<ChangeSet, ConsensusError> {
            Ok(new_changes)
        }

        fn changes_for_persist(
            &self,
            digest: Digest,
        ) -> Result<(Vec<Digest>, ChangeSet), ConsensusError> {
            let snapshot = self
                .snapshots
                .read()
                .unwrap()
                .get(&digest)
                .cloned()
                .ok_or(ConsensusError::SnapshotNotFound(digest))?;
            Ok((vec![digest], snapshot.changes))
        }
    }

    #[derive(Clone)]
    struct MockExecutor;

    impl BlockExecutor<MockStateDb> for MockExecutor {
        type Tx = Bytes;

        fn execute(
            &self,
            _state: &MockStateDb,
            _context: &BlockContext,
            txs: &[Self::Tx],
        ) -> Result<ExecutionOutcome, kora_executor::ExecutionError> {
            Ok(ExecutionOutcome {
                changes: ChangeSet::new(),
                receipts: Vec::new(),
                gas_used: txs.len() as u64 * 21000,
            })
        }

        fn validate_header(&self, _header: &Header) -> Result<(), kora_executor::ExecutionError> {
            Ok(())
        }
    }

    fn digest_from_byte(byte: u8) -> Digest {
        Digest::from([byte; 32])
    }

    fn parent_block() -> Block {
        Block {
            parent: kora_domain::BlockId(B256::ZERO),
            height: 0,
            prevrandao: B256::ZERO,
            state_root: StateRoot(B256::ZERO),
            txs: Vec::new(),
        }
    }

    #[test]
    fn proposal_builder_new() {
        let state = MockStateDb::new();
        let mempool = MockMempool::new();
        let snapshots = MockSnapshotStore::new();
        let executor = MockExecutor;

        let builder = ProposalBuilder::new(state, mempool, snapshots, executor);
        assert_eq!(builder.max_txs, ProposalBuilder::<MockStateDb, MockMempool, MockSnapshotStore, MockExecutor>::DEFAULT_MAX_TXS);
    }

    #[test]
    fn proposal_builder_with_max_txs() {
        let state = MockStateDb::new();
        let mempool = MockMempool::new();
        let snapshots = MockSnapshotStore::new();
        let executor = MockExecutor;

        let builder = ProposalBuilder::new(state, mempool, snapshots, executor).with_max_txs(50);
        assert_eq!(builder.max_txs, 50);
    }

    #[test]
    fn proposal_builder_missing_parent() {
        let state = MockStateDb::new();
        let mempool = MockMempool::new();
        let snapshots = MockSnapshotStore::new();
        let executor = MockExecutor;

        let builder = ProposalBuilder::new(state, mempool, snapshots, executor);

        let parent = parent_block();
        let result = builder.build_proposal(&parent, B256::ZERO);

        assert!(matches!(result, Err(ConsensusError::SnapshotNotFound(_))));
    }

    #[test]
    fn proposal_builder_empty_block() {
        let state = MockStateDb::new();
        let mempool = MockMempool::new();
        let snapshots = MockSnapshotStore::new();
        let executor = MockExecutor;

        // Insert parent snapshot
        let parent = parent_block();
        let parent_digest = parent.commitment();
        let parent_snapshot = Snapshot::new(
            None,
            MockStateDb::new(),
            StateRoot(B256::ZERO),
            ChangeSet::new(),
            BTreeSet::new(),
        );
        snapshots.insert(parent_digest, parent_snapshot);

        let builder = ProposalBuilder::new(state, mempool, snapshots, executor);

        let result = builder.build_proposal(&parent, B256::ZERO);
        assert!(result.is_ok());

        let (block, snapshot) = result.unwrap();
        assert_eq!(block.txs.len(), 0);
        assert_eq!(block.parent, parent.id());
        assert_eq!(snapshot.parent, Some(parent_digest));
    }

    #[test]
    fn proposal_builder_with_transactions() {
        let state = MockStateDb::new();
        let mempool = MockMempool::new();
        let snapshots = MockSnapshotStore::new();
        let executor = MockExecutor;

        // Add transactions to mempool
        mempool.add(Tx::new(vec![1, 2, 3].into()));
        mempool.add(Tx::new(vec![4, 5, 6].into()));

        // Insert parent snapshot
        let parent = parent_block();
        let parent_digest = parent.commitment();
        let parent_snapshot = Snapshot::new(
            None,
            MockStateDb::new(),
            StateRoot(B256::ZERO),
            ChangeSet::new(),
            BTreeSet::new(),
        );
        snapshots.insert(parent_digest, parent_snapshot);

        let builder = ProposalBuilder::new(state, mempool, snapshots, executor);

        let result = builder.build_proposal(&parent, B256::repeat_byte(0xAB));
        assert!(result.is_ok());

        let (block, snapshot) = result.unwrap();
        assert_eq!(block.txs.len(), 2);
        assert_eq!(block.prevrandao, B256::repeat_byte(0xAB));
        assert!(snapshot.parent.is_some());
    }

    #[test]
    fn proposal_builder_respects_max_txs() {
        let state = MockStateDb::new();
        let mempool = MockMempool::new();
        let snapshots = MockSnapshotStore::new();
        let executor = MockExecutor;

        // Add many transactions
        for i in 0..100 {
            mempool.add(Tx::new(vec![i].into()));
        }

        // Insert parent snapshot
        let parent = parent_block();
        let parent_digest = parent.commitment();
        let parent_snapshot = Snapshot::new(
            None,
            MockStateDb::new(),
            StateRoot(B256::ZERO),
            ChangeSet::new(),
            BTreeSet::new(),
        );
        snapshots.insert(parent_digest, parent_snapshot);

        let builder = ProposalBuilder::new(state, mempool, snapshots, executor).with_max_txs(10);

        let result = builder.build_proposal(&parent, B256::ZERO);
        assert!(result.is_ok());

        let (block, _) = result.unwrap();
        assert_eq!(block.txs.len(), 10);
    }

    #[test]
    fn tx_id_computation() {
        let tx = Tx::new(vec![1, 2, 3, 4, 5].into());
        let id = tx.id();
        assert_eq!(id, tx.id());
    }

    #[test]
    fn proposal_builder_state_root_in_header() {
        let state = MockStateDb::new();
        let mempool = MockMempool::new();
        let snapshots = MockSnapshotStore::new();
        let executor = MockExecutor;

        // Insert parent snapshot
        let parent = parent_block();
        let parent_digest = parent.commitment();
        let parent_snapshot = Snapshot::new(
            None,
            MockStateDb::new(),
            StateRoot(B256::ZERO),
            ChangeSet::new(),
            BTreeSet::new(),
        );
        snapshots.insert(parent_digest, parent_snapshot);

        let builder = ProposalBuilder::new(state, mempool, snapshots, executor);

        let (block, snapshot) = builder.build_proposal(&parent, B256::ZERO).unwrap();

        // MockStateDb::compute_root returns B256::repeat_byte(0x42)
        let expected_root = StateRoot(B256::repeat_byte(0x42));
        assert_eq!(block.state_root, expected_root);
        assert_eq!(snapshot.state_root, expected_root);
    }

    #[test]
    fn gas_used_field() {
        let state = MockStateDb::new();
        let mempool = MockMempool::new();
        let snapshots = MockSnapshotStore::new();
        let executor = MockExecutor;

        // Add transactions
        mempool.add(Tx::new(vec![1].into()));
        mempool.add(Tx::new(vec![2].into()));
        mempool.add(Tx::new(vec![3].into()));

        // Insert parent snapshot
        let parent = parent_block();
        let parent_digest = parent.commitment();
        let parent_snapshot = Snapshot::new(
            None,
            MockStateDb::new(),
            StateRoot(B256::ZERO),
            ChangeSet::new(),
            BTreeSet::new(),
        );
        snapshots.insert(parent_digest, parent_snapshot);

        let builder = ProposalBuilder::new(state, mempool, snapshots, executor);

        let (block, _) = builder.build_proposal(&parent, B256::ZERO).unwrap();

        assert_eq!(block.txs.len(), 3);
    }

    #[test]
    fn parent_tx_ids_excludes_parent_txs() {
        let state = MockStateDb::new();
        let mempool = MockMempool::new();
        let snapshots = MockSnapshotStore::new();
        let executor = MockExecutor;

        let tx = Tx::new(vec![9].into());
        let parent = Block {
            parent: kora_domain::BlockId(B256::ZERO),
            height: 0,
            prevrandao: B256::ZERO,
            state_root: StateRoot(B256::ZERO),
            txs: vec![tx.clone()],
        };
        let parent_digest = parent.commitment();
        let parent_snapshot = Snapshot::new(
            None,
            MockStateDb::new(),
            StateRoot(B256::ZERO),
            ChangeSet::new(),
            BTreeSet::from([tx.id()]),
        );
        snapshots.insert(parent_digest, parent_snapshot);

        mempool.add(tx);

        let builder = ProposalBuilder::new(state, mempool, snapshots, executor);
        let result = builder.build_proposal(&parent, B256::ZERO).unwrap();

        assert!(result.0.txs.is_empty());
    }

    #[test]
    fn digest_from_byte_helper() {
        let digest = digest_from_byte(0xAA);
        assert_eq!(digest_from_byte(0xAA), digest);
    }
}
