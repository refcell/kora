//! Node-local state for the REVM chain example.
//!
//! Threshold-simplex orders only block digests. Full blocks are verified by the application and
//! disseminated/backfilled by `commonware_consensus::marshal`. This module holds the minimal
//! shared state needed by the example:
//! - a mempool of submitted transactions,
//! - per-block execution snapshots (QMDB base state + merged change set overlay) keyed by the
//!   consensus digest, and
//! - a per-digest seed hash used to populate the next block's `prevrandao`.
//!
//! The simulation harness queries this state through `crate::application::NodeHandle`.

use std::{collections::BTreeSet, sync::Arc};

use alloy_evm::revm::primitives::{Address, B256, U256};
use commonware_cryptography::Committable as _;
use commonware_runtime::{Metrics as _, buffer::PoolRef, tokio};
use futures::{channel::mpsc::UnboundedReceiver, lock::Mutex};
use kora_consensus::{
    Mempool as _, SeedTracker as _, Snapshot, SnapshotStore as _,
    components::{InMemoryMempool, InMemorySeedTracker, InMemorySnapshotStore},
};
use kora_domain::{
    Block, BlockId, ConsensusDigest, LedgerEvent, LedgerEvents, StateRoot, Tx, TxId,
};
use kora_overlay::OverlayState;
use kora_qmdb_ledger::{QmdbChangeSet, QmdbConfig, QmdbLedger, QmdbState};
use kora_traits::{StateDbRead, StateDbWrite};

type LedgerSnapshot = Snapshot<OverlayState<QmdbState>>;

fn tx_ids(txs: &[Tx]) -> BTreeSet<TxId> {
    txs.iter().map(Tx::id).collect()
}
#[derive(Clone)]
/// Ledger view that owns the mutexed execution state.
pub(crate) struct LedgerView {
    /// Mutex-protected running state.
    inner: Arc<Mutex<LedgerState>>,
    /// Genesis block stored so the automaton can replay from height 0.
    genesis_block: Block,
}

/// Internal ledger state guarded by the mutex inside `LedgerView`.
pub(crate) struct LedgerState {
    /// Pending transactions that are not yet included in finalized blocks.
    mempool: InMemoryMempool,
    /// Execution snapshots indexed by digest so we can replay ancestors.
    snapshots: InMemorySnapshotStore<OverlayState<QmdbState>>,
    /// Cached seeds for each digest used to compute prevrandao.
    seeds: InMemorySeedTracker,
    /// Underlying QMDB ledger service for persistence.
    qmdb: QmdbLedger,
}

/// Minimal mempool helper that avoids duplicating logics across services.
impl LedgerView {
    pub(crate) async fn init(
        context: tokio::Context,
        buffer_pool: PoolRef,
        partition_prefix: String,
        genesis_alloc: Vec<(Address, U256)>,
    ) -> anyhow::Result<Self> {
        let qmdb = QmdbLedger::init(
            context.with_label("qmdb"),
            QmdbConfig::new(partition_prefix, buffer_pool),
            genesis_alloc,
        )
        .await?;
        let genesis_root = qmdb.root().await?;

        let genesis_block = Block {
            parent: BlockId(B256::ZERO),
            height: 0,
            prevrandao: B256::ZERO,
            state_root: genesis_root,
            txs: Vec::new(),
        };
        let genesis_digest = genesis_block.commitment();
        let state = OverlayState::new(qmdb.state(), QmdbChangeSet::default());
        let snapshots = InMemorySnapshotStore::new();
        let genesis_snapshot = Snapshot::new(
            None,
            state,
            genesis_block.state_root,
            QmdbChangeSet::default(),
            BTreeSet::new(),
        );
        snapshots.insert(genesis_digest, genesis_snapshot);
        snapshots.mark_persisted(&[genesis_digest]);

        Ok(Self {
            inner: Arc::new(Mutex::new(LedgerState {
                mempool: InMemoryMempool::new(),
                snapshots,
                seeds: InMemorySeedTracker::new(genesis_digest),
                qmdb,
            })),
            genesis_block,
        })
    }

    pub(crate) fn genesis_block(&self) -> Block {
        self.genesis_block.clone()
    }

    pub(crate) async fn submit_tx(&self, tx: Tx) -> bool {
        let inner = self.inner.lock().await;
        inner.mempool.insert(tx)
    }

    pub(crate) async fn query_balance(
        &self,
        digest: ConsensusDigest,
        address: Address,
    ) -> Option<U256> {
        let snapshot = {
            let inner = self.inner.lock().await;
            inner.snapshots.get(&digest)
        }?;
        snapshot.state.balance(&address).await.ok()
    }

    pub(crate) async fn query_state_root(&self, digest: ConsensusDigest) -> Option<StateRoot> {
        let inner = self.inner.lock().await;
        inner.snapshots.get(&digest).map(|snapshot| snapshot.state_root)
    }

    pub(crate) async fn query_seed(&self, digest: ConsensusDigest) -> Option<B256> {
        let inner = self.inner.lock().await;
        inner.seeds.get(&digest)
    }

    pub(crate) async fn seed_for_parent(&self, parent: ConsensusDigest) -> Option<B256> {
        let inner = self.inner.lock().await;
        inner.seeds.get(&parent)
    }

    pub(crate) async fn set_seed(&self, digest: ConsensusDigest, seed_hash: B256) {
        let inner = self.inner.lock().await;
        inner.seeds.insert(digest, seed_hash);
    }

    pub(crate) async fn parent_snapshot(&self, parent: ConsensusDigest) -> Option<LedgerSnapshot> {
        let inner = self.inner.lock().await;
        inner.snapshots.get(&parent)
    }

    pub(crate) async fn insert_snapshot(
        &self,
        digest: ConsensusDigest,
        parent: ConsensusDigest,
        state: OverlayState<QmdbState>,
        root: StateRoot,
        qmdb_changes: QmdbChangeSet,
        txs: &[Tx],
    ) {
        let inner = self.inner.lock().await;
        let ids = tx_ids(txs);
        inner.snapshots.insert(digest, Snapshot::new(Some(parent), state, root, qmdb_changes, ids));
    }

    pub(crate) async fn cache_snapshot(&self, digest: ConsensusDigest, snapshot: LedgerSnapshot) {
        let inner = self.inner.lock().await;
        inner.snapshots.insert(digest, snapshot);
    }

    pub(crate) async fn proposal_components(
        &self,
    ) -> (OverlayState<QmdbState>, InMemoryMempool, InMemorySnapshotStore<OverlayState<QmdbState>>)
    {
        let inner = self.inner.lock().await;
        let root_state = OverlayState::new(inner.qmdb.state(), QmdbChangeSet::default());
        (root_state, inner.mempool.clone(), inner.snapshots.clone())
    }

    /// Compute a preview root as if all unpersisted ancestors plus `changes` were applied.
    ///
    /// Note: QMDB roots include commit metadata, so persisted roots can differ from this preview.
    #[cfg(test)]
    pub(crate) async fn compute_qmdb_root(
        &self,
        parent: ConsensusDigest,
        changes: QmdbChangeSet,
    ) -> anyhow::Result<StateRoot> {
        self.compute_root_from_store(parent, changes).await
    }

    pub(crate) async fn compute_root_from_store(
        &self,
        parent: ConsensusDigest,
        changes: QmdbChangeSet,
    ) -> anyhow::Result<StateRoot> {
        let (changes, state) = {
            let inner = self.inner.lock().await;
            let changes = inner.snapshots.merged_changes(parent, changes)?;
            (changes, inner.qmdb.state())
        };
        let root = state.compute_root(&changes).await?;
        Ok(StateRoot(root))
    }

    /// Persist `digest` and any missing ancestors to QMDB.
    ///
    /// Returns `Ok(true)` if a new commit happened, or `Ok(false)` if the digest is already
    /// persisted or currently being persisted by another task.
    pub(crate) async fn persist_snapshot(&self, digest: ConsensusDigest) -> anyhow::Result<bool> {
        let (changes, qmdb, chain) = {
            let inner = self.inner.lock().await;
            let (chain, changes) = inner.snapshots.changes_for_persist(digest)?;
            if chain.is_empty() {
                return Ok(false);
            }
            if !inner.snapshots.can_persist_chain(&chain) {
                return Ok(false);
            }
            inner.snapshots.mark_persisting_chain(&chain);
            (changes, inner.qmdb.clone(), chain)
        };

        let result = qmdb.commit_changes(changes).await;
        let inner = self.inner.lock().await;
        inner.snapshots.clear_persisting_chain(&chain);
        match result {
            Ok(_) => {
                inner.snapshots.mark_persisted(&chain);
                Ok(true)
            }
            Err(err) => Err(err.into()),
        }
    }

    pub(crate) async fn prune_mempool(&self, txs: &[Tx]) {
        let inner = self.inner.lock().await;
        let tx_ids: Vec<TxId> = txs.iter().map(Tx::id).collect();
        inner.mempool.prune(&tx_ids);
    }
}

#[derive(Clone)]
/// Domain service that exposes high-level ledger commands.
pub(crate) struct LedgerService {
    view: LedgerView,
    events: LedgerEvents,
}

impl LedgerService {
    pub(crate) fn new(view: LedgerView) -> Self {
        Self { view, events: LedgerEvents::new() }
    }

    fn publish(&self, event: LedgerEvent) {
        self.events.publish(event);
    }

    #[allow(dead_code)]
    pub(crate) fn subscribe(&self) -> UnboundedReceiver<LedgerEvent> {
        self.events.subscribe()
    }

    pub(crate) fn genesis_block(&self) -> Block {
        self.view.genesis_block()
    }

    pub(crate) async fn submit_tx(&self, tx: Tx) -> bool {
        let tx_id = tx.id();
        let inserted = self.view.submit_tx(tx).await;
        if inserted {
            self.publish(LedgerEvent::TransactionSubmitted(tx_id));
        }
        inserted
    }

    pub(crate) async fn query_balance(
        &self,
        digest: ConsensusDigest,
        address: Address,
    ) -> Option<U256> {
        self.view.query_balance(digest, address).await
    }

    pub(crate) async fn query_state_root(&self, digest: ConsensusDigest) -> Option<StateRoot> {
        self.view.query_state_root(digest).await
    }

    pub(crate) async fn query_seed(&self, digest: ConsensusDigest) -> Option<B256> {
        self.view.query_seed(digest).await
    }

    pub(crate) async fn seed_for_parent(&self, parent: ConsensusDigest) -> Option<B256> {
        self.view.seed_for_parent(parent).await
    }

    pub(crate) async fn set_seed(&self, digest: ConsensusDigest, seed_hash: B256) {
        self.view.set_seed(digest, seed_hash).await;
        self.publish(LedgerEvent::SeedUpdated(digest, seed_hash));
    }

    pub(crate) async fn parent_snapshot(&self, parent: ConsensusDigest) -> Option<LedgerSnapshot> {
        self.view.parent_snapshot(parent).await
    }

    pub(crate) async fn insert_snapshot(
        &self,
        digest: ConsensusDigest,
        parent: ConsensusDigest,
        state: OverlayState<QmdbState>,
        root: StateRoot,
        changes: QmdbChangeSet,
        txs: &[Tx],
    ) {
        self.view.insert_snapshot(digest, parent, state, root, changes, txs).await;
    }

    pub(crate) async fn cache_snapshot(&self, digest: ConsensusDigest, snapshot: LedgerSnapshot) {
        self.view.cache_snapshot(digest, snapshot).await;
    }

    pub(crate) async fn proposal_components(
        &self,
    ) -> (OverlayState<QmdbState>, InMemoryMempool, InMemorySnapshotStore<OverlayState<QmdbState>>)
    {
        self.view.proposal_components().await
    }

    #[cfg(test)]
    pub(crate) async fn compute_root(
        &self,
        parent: ConsensusDigest,
        changes: QmdbChangeSet,
    ) -> anyhow::Result<StateRoot> {
        self.view.compute_qmdb_root(parent, changes).await
    }

    pub(crate) async fn compute_root_from_store(
        &self,
        parent: ConsensusDigest,
        changes: QmdbChangeSet,
    ) -> anyhow::Result<StateRoot> {
        self.view.compute_root_from_store(parent, changes).await
    }

    pub(crate) async fn persist_snapshot(&self, digest: ConsensusDigest) -> anyhow::Result<()> {
        let persisted = self.view.persist_snapshot(digest).await?;
        if persisted {
            self.publish(LedgerEvent::SnapshotPersisted(digest));
        }
        Ok(())
    }

    pub(crate) async fn prune_mempool(&self, txs: &[Tx]) {
        self.view.prune_mempool(txs).await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use alloy_consensus::Header;
    use alloy_evm::revm::{Database as _, database::CacheDB};
    use alloy_primitives::{Address, B256, Bytes, U256};
    use commonware_cryptography::Committable as _;
    use commonware_runtime::{Runner, buffer::PoolRef, tokio};
    use commonware_utils::{NZU16, NZUsize};
    use k256::ecdsa::SigningKey;
    use kora_consensus::SnapshotStore as _;
    use kora_domain::{Block, ConsensusDigest, Tx, evm::Evm};
    use kora_executor::{BlockContext, BlockExecutor, RevmExecutor};
    use kora_qmdb_ledger::QmdbRefDb;

    use super::{LedgerService, LedgerSnapshot, LedgerView, OverlayState};
    use crate::chain::CHAIN_ID;

    type RevmDb = CacheDB<QmdbRefDb>;

    static PARTITION_COUNTER: AtomicUsize = AtomicUsize::new(0);

    const BUFFER_BLOCK_BYTES: u16 = 16_384;
    const BUFFER_BLOCK_COUNT: usize = 10_000;
    const GENESIS_BALANCE: u64 = 1_000_000;
    const DUPLICATE_BALANCE: u64 = 500_000;
    const TRANSFER_ONE: u64 = 10;
    const TRANSFER_TWO: u64 = 5;
    const TRANSFER_DUPLICATE: u64 = 1;
    const GAS_LIMIT_TRANSFER: u64 = 21_000;
    const HEIGHT_ONE: u64 = 1;
    const HEIGHT_TWO: u64 = 2;
    const PREVRANDAO: B256 = B256::ZERO;
    const FROM_BYTE_A: u8 = 0x11;
    const TO_BYTE_A: u8 = 0x22;
    const FROM_BYTE_B: u8 = 0x33;
    const TO_BYTE_B: u8 = 0x44;

    struct LedgerSetup {
        ledger: LedgerView,
        service: LedgerService,
        genesis: Block,
        genesis_digest: ConsensusDigest,
    }

    struct BuiltBlock {
        block: Block,
        digest: ConsensusDigest,
    }

    fn key_from_byte(byte: u8) -> SigningKey {
        let mut bytes = [0u8; 32];
        bytes[0] = byte.max(1);
        SigningKey::from_bytes(&bytes.into()).expect("valid key")
    }

    fn next_partition(prefix: &str) -> String {
        let id = PARTITION_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{prefix}-{id}")
    }

    fn test_buffer_pool() -> PoolRef {
        PoolRef::new(NZU16!(BUFFER_BLOCK_BYTES), NZUsize!(BUFFER_BLOCK_COUNT))
    }

    fn transfer_tx(from_key: &SigningKey, to: Address, value: u64, nonce: u64) -> Tx {
        Evm::sign_eip1559_transfer(
            from_key,
            CHAIN_ID,
            to,
            U256::from(value),
            nonce,
            GAS_LIMIT_TRANSFER,
        )
    }

    fn block_context(height: u64, prevrandao: B256) -> BlockContext {
        let header = Header {
            number: height,
            timestamp: height,
            gas_limit: 30_000_000,
            beneficiary: Address::ZERO,
            base_fee_per_gas: Some(0),
            ..Default::default()
        };
        BlockContext::new(header, prevrandao)
    }

    async fn setup_ledger(
        context: tokio::Context,
        partition_prefix: &str,
        allocations: Vec<(Address, U256)>,
    ) -> LedgerSetup {
        let ledger = LedgerView::init(
            context,
            test_buffer_pool(),
            next_partition(partition_prefix),
            allocations,
        )
        .await
        .expect("init ledger");
        let service = LedgerService::new(ledger.clone());
        let genesis = service.genesis_block();
        let genesis_digest = genesis.commitment();
        LedgerSetup { ledger, service, genesis, genesis_digest }
    }

    async fn build_block_snapshot(
        service: &LedgerService,
        parent: &Block,
        parent_snapshot: LedgerSnapshot,
        height: u64,
        txs: Vec<Tx>,
    ) -> BuiltBlock {
        let executor = RevmExecutor::new(CHAIN_ID);
        let context = block_context(height, PREVRANDAO);
        let txs_bytes: Vec<Bytes> = txs.iter().map(|tx| tx.bytes.clone()).collect();
        let outcome =
            executor.execute(&parent_snapshot.state, &context, &txs_bytes).expect("execute txs");
        let merged_changes = parent_snapshot.state.merge_changes(outcome.changes.clone());
        let parent_digest = parent.commitment();
        let root = service
            .compute_root(parent_digest, outcome.changes.clone())
            .await
            .expect("compute root");
        let block =
            Block { parent: parent.id(), height, prevrandao: PREVRANDAO, state_root: root, txs };
        let digest = block.commitment();
        let next_state = OverlayState::new(parent_snapshot.state.base(), merged_changes);
        service
            .insert_snapshot(digest, parent_digest, next_state, root, outcome.changes, &block.txs)
            .await;
        BuiltBlock { block, digest }
    }

    #[test]
    fn persist_snapshot_merges_unpersisted_ancestors() {
        // Tokio runtime required for WrapDatabaseAsync in the QMDB adapter.
        let executor = tokio::Runner::default();
        executor.start(|context| async move {
            // Arrange
            let from_key = key_from_byte(FROM_BYTE_A);
            let to_key = key_from_byte(TO_BYTE_A);
            let from = Evm::address_from_key(&from_key);
            let to = Evm::address_from_key(&to_key);
            let setup = setup_ledger(
                context,
                "revm-ledger-merge",
                vec![(from, U256::from(GENESIS_BALANCE)), (to, U256::ZERO)],
            )
            .await;
            let parent_snapshot = setup
                .service
                .parent_snapshot(setup.genesis_digest)
                .await
                .expect("genesis snapshot");
            let block1 = build_block_snapshot(
                &setup.service,
                &setup.genesis,
                parent_snapshot,
                HEIGHT_ONE,
                vec![transfer_tx(&from_key, to, TRANSFER_ONE, 0)],
            )
            .await;
            let parent_snapshot =
                setup.service.parent_snapshot(block1.digest).await.expect("parent snapshot");
            let block2 = build_block_snapshot(
                &setup.service,
                &block1.block,
                parent_snapshot,
                HEIGHT_TWO,
                vec![transfer_tx(&from_key, to, TRANSFER_TWO, 1)],
            )
            .await;

            // Act
            let persisted =
                setup.ledger.persist_snapshot(block2.digest).await.expect("persist snapshot");

            // Assert
            assert!(persisted, "expected merged persistence");

            let qmdb = {
                let inner = setup.ledger.inner.lock().await;
                inner.qmdb.clone()
            };
            qmdb.root().await.expect("qmdb root");

            let mut persisted_db = RevmDb::new(qmdb.database().expect("qmdb db"));
            let from_info = persisted_db.basic(from).expect("sender account");
            let to_info = persisted_db.basic(to).expect("recipient account");
            let expected_from = GENESIS_BALANCE - TRANSFER_ONE - TRANSFER_TWO;
            let expected_to = TRANSFER_ONE + TRANSFER_TWO;
            assert_eq!(from_info.expect("sender exists").balance, U256::from(expected_from));
            assert_eq!(to_info.expect("recipient exists").balance, U256::from(expected_to));

            let inner = setup.ledger.inner.lock().await;
            assert!(inner.snapshots.is_persisted(&block1.digest));
            assert!(inner.snapshots.is_persisted(&block2.digest));
        });
    }

    #[test]
    fn persist_snapshot_duplicate_is_noop() {
        // Tokio runtime required for WrapDatabaseAsync in the QMDB adapter.
        let executor = tokio::Runner::default();
        executor.start(|context| async move {
            // Arrange
            let from_key = key_from_byte(FROM_BYTE_B);
            let to_key = key_from_byte(TO_BYTE_B);
            let from = Evm::address_from_key(&from_key);
            let to = Evm::address_from_key(&to_key);
            let setup = setup_ledger(
                context,
                "revm-ledger-dup",
                vec![(from, U256::from(DUPLICATE_BALANCE)), (to, U256::ZERO)],
            )
            .await;
            let parent_snapshot = setup
                .service
                .parent_snapshot(setup.genesis_digest)
                .await
                .expect("genesis snapshot");
            let block = build_block_snapshot(
                &setup.service,
                &setup.genesis,
                parent_snapshot,
                HEIGHT_ONE,
                vec![transfer_tx(&from_key, to, TRANSFER_DUPLICATE, 0)],
            )
            .await;

            // Act
            let first =
                setup.ledger.persist_snapshot(block.digest).await.expect("persist snapshot");
            assert!(first);

            let second =
                setup.ledger.persist_snapshot(block.digest).await.expect("persist snapshot");

            // Assert
            assert!(!second, "duplicate persist should be a no-op");
        });
    }
}
