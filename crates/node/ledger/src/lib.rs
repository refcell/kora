//! Ledger services for Kora nodes.

#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::{collections::BTreeSet, fmt, sync::Arc};

use alloy_primitives::{Address, B256, U256};
use commonware_cryptography::Committable as _;
use commonware_runtime::{Metrics as _, buffer::PoolRef, tokio};
use futures::{channel::mpsc::UnboundedReceiver, lock::Mutex};
use kora_consensus::{
    ConsensusError, Mempool as _, SeedTracker as _, Snapshot, SnapshotStore as _,
    components::{InMemoryMempool, InMemorySeedTracker, InMemorySnapshotStore},
};
use kora_domain::{
    Block, BlockId, ConsensusDigest, LedgerEvent, LedgerEvents, StateRoot, Tx, TxId,
};
use kora_overlay::OverlayState;
use kora_qmdb_ledger::{Error as QmdbError, QmdbChangeSet, QmdbConfig, QmdbLedger, QmdbState};
use kora_traits::{StateDbError, StateDbRead, StateDbWrite};
use thiserror::Error;

/// Snapshot type used by the ledger.
pub type LedgerSnapshot = Snapshot<OverlayState<QmdbState>>;

fn tx_ids(txs: &[Tx]) -> BTreeSet<TxId> {
    txs.iter().map(Tx::id).collect()
}

/// Errors surfaced by ledger services.
#[derive(Debug, Error)]
pub enum LedgerError {
    /// QMDB-backed storage error.
    #[error("qmdb error: {0}")]
    Qmdb(#[from] QmdbError),
    /// Snapshot store or consensus component error.
    #[error("consensus error: {0}")]
    Consensus(#[from] ConsensusError),
    /// State database error.
    #[error("state db error: {0}")]
    StateDb(#[from] StateDbError),
}

/// Result alias for ledger operations.
pub type LedgerResult<T> = Result<T, LedgerError>;

/// Ledger view that owns the mutexed execution state.
#[derive(Clone)]
pub struct LedgerView {
    /// Mutex-protected running state.
    inner: Arc<Mutex<LedgerState>>,
    /// Genesis block stored so the automaton can replay from height 0.
    genesis_block: Block,
}

impl fmt::Debug for LedgerView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LedgerView").finish_non_exhaustive()
    }
}

/// Internal ledger state guarded by the mutex inside `LedgerView`.
struct LedgerState {
    /// Pending transactions that are not yet included in finalized blocks.
    mempool: InMemoryMempool,
    /// Execution snapshots indexed by digest so we can replay ancestors.
    snapshots: InMemorySnapshotStore<OverlayState<QmdbState>>,
    /// Cached seeds for each digest used to compute prevrandao.
    seeds: InMemorySeedTracker,
    /// Underlying QMDB ledger service for persistence.
    qmdb: QmdbLedger,
}

impl LedgerView {
    /// Initialize a ledger view with a QMDB backend built from the provided settings.
    pub async fn init(
        context: tokio::Context,
        buffer_pool: PoolRef,
        partition_prefix: String,
        genesis_alloc: Vec<(Address, U256)>,
    ) -> LedgerResult<Self> {
        let config = QmdbConfig::new(partition_prefix, buffer_pool);
        Self::init_with_config(context, config, genesis_alloc).await
    }

    /// Initialize a ledger view with an explicit QMDB configuration.
    pub async fn init_with_config(
        context: tokio::Context,
        config: QmdbConfig,
        genesis_alloc: Vec<(Address, U256)>,
    ) -> LedgerResult<Self> {
        let qmdb = QmdbLedger::init(context.with_label("qmdb"), config, genesis_alloc).await?;
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

    /// Return the genesis block of this ledger.
    pub fn genesis_block(&self) -> Block {
        self.genesis_block.clone()
    }

    /// Submit a transaction into the mempool.
    pub async fn submit_tx(&self, tx: Tx) -> bool {
        let inner = self.inner.lock().await;
        inner.mempool.insert(tx)
    }

    /// Query a balance at the given digest.
    pub async fn query_balance(&self, digest: ConsensusDigest, address: Address) -> Option<U256> {
        let snapshot = {
            let inner = self.inner.lock().await;
            inner.snapshots.get(&digest)
        }?;
        snapshot.state.balance(&address).await.ok()
    }

    /// Query a state root at the given digest.
    pub async fn query_state_root(&self, digest: ConsensusDigest) -> Option<StateRoot> {
        let inner = self.inner.lock().await;
        inner.snapshots.get(&digest).map(|snapshot| snapshot.state_root)
    }

    /// Query the cached seed at the given digest.
    pub async fn query_seed(&self, digest: ConsensusDigest) -> Option<B256> {
        let inner = self.inner.lock().await;
        inner.seeds.get(&digest)
    }

    /// Return the seed associated with a parent digest.
    pub async fn seed_for_parent(&self, parent: ConsensusDigest) -> Option<B256> {
        let inner = self.inner.lock().await;
        inner.seeds.get(&parent)
    }

    /// Store the seed hash for a digest.
    pub async fn set_seed(&self, digest: ConsensusDigest, seed_hash: B256) {
        let inner = self.inner.lock().await;
        inner.seeds.insert(digest, seed_hash);
    }

    /// Fetch the parent snapshot for a given digest.
    pub async fn parent_snapshot(&self, parent: ConsensusDigest) -> Option<LedgerSnapshot> {
        let inner = self.inner.lock().await;
        inner.snapshots.get(&parent)
    }

    /// Insert a snapshot for a block digest.
    pub async fn insert_snapshot(
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

    /// Cache a snapshot that has already been constructed.
    pub async fn cache_snapshot(&self, digest: ConsensusDigest, snapshot: LedgerSnapshot) {
        let inner = self.inner.lock().await;
        inner.snapshots.insert(digest, snapshot);
    }

    /// Fetch the components needed to build a proposal.
    pub async fn proposal_components(
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
    pub async fn compute_root(
        &self,
        parent: ConsensusDigest,
        changes: QmdbChangeSet,
    ) -> LedgerResult<StateRoot> {
        self.compute_root_from_store(parent, changes).await
    }

    /// Compute a root using the persisted QMDB store plus any pending changes.
    pub async fn compute_root_from_store(
        &self,
        parent: ConsensusDigest,
        changes: QmdbChangeSet,
    ) -> LedgerResult<StateRoot> {
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
    pub async fn persist_snapshot(&self, digest: ConsensusDigest) -> LedgerResult<bool> {
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

    /// Remove transactions that are included in a block from the mempool.
    pub async fn prune_mempool(&self, txs: &[Tx]) {
        let inner = self.inner.lock().await;
        let tx_ids: Vec<TxId> = txs.iter().map(Tx::id).collect();
        inner.mempool.prune(&tx_ids);
    }
}

/// Domain service that exposes high-level ledger commands.
#[derive(Clone)]
pub struct LedgerService {
    view: LedgerView,
    events: LedgerEvents,
}

impl fmt::Debug for LedgerService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LedgerService").finish_non_exhaustive()
    }
}

impl LedgerService {
    /// Create a new ledger service from a ledger view.
    pub fn new(view: LedgerView) -> Self {
        Self { view, events: LedgerEvents::new() }
    }

    fn publish(&self, event: LedgerEvent) {
        self.events.publish(event);
    }

    /// Subscribe to ledger events.
    pub fn subscribe(&self) -> UnboundedReceiver<LedgerEvent> {
        self.events.subscribe()
    }

    /// Return the genesis block.
    pub fn genesis_block(&self) -> Block {
        self.view.genesis_block()
    }

    /// Submit a transaction and emit events.
    pub async fn submit_tx(&self, tx: Tx) -> bool {
        let tx_id = tx.id();
        let inserted = self.view.submit_tx(tx).await;
        if inserted {
            self.publish(LedgerEvent::TransactionSubmitted(tx_id));
        }
        inserted
    }

    /// Query a balance at the given digest.
    pub async fn query_balance(&self, digest: ConsensusDigest, address: Address) -> Option<U256> {
        self.view.query_balance(digest, address).await
    }

    /// Query the stored state root at the given digest.
    pub async fn query_state_root(&self, digest: ConsensusDigest) -> Option<StateRoot> {
        self.view.query_state_root(digest).await
    }

    /// Query the cached seed at the given digest.
    pub async fn query_seed(&self, digest: ConsensusDigest) -> Option<B256> {
        self.view.query_seed(digest).await
    }

    /// Query the seed for a parent digest.
    pub async fn seed_for_parent(&self, parent: ConsensusDigest) -> Option<B256> {
        self.view.seed_for_parent(parent).await
    }

    /// Store the seed for a digest and publish an event.
    pub async fn set_seed(&self, digest: ConsensusDigest, seed_hash: B256) {
        self.view.set_seed(digest, seed_hash).await;
        self.publish(LedgerEvent::SeedUpdated(digest, seed_hash));
    }

    /// Fetch the snapshot of a parent digest.
    pub async fn parent_snapshot(&self, parent: ConsensusDigest) -> Option<LedgerSnapshot> {
        self.view.parent_snapshot(parent).await
    }

    /// Insert a new snapshot.
    pub async fn insert_snapshot(
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

    /// Cache a fully constructed snapshot.
    pub async fn cache_snapshot(&self, digest: ConsensusDigest, snapshot: LedgerSnapshot) {
        self.view.cache_snapshot(digest, snapshot).await;
    }

    /// Fetch proposal components.
    pub async fn proposal_components(
        &self,
    ) -> (OverlayState<QmdbState>, InMemoryMempool, InMemorySnapshotStore<OverlayState<QmdbState>>)
    {
        self.view.proposal_components().await
    }

    /// Compute a preview root (test-only helper).
    #[cfg(test)]
    pub async fn compute_root(
        &self,
        parent: ConsensusDigest,
        changes: QmdbChangeSet,
    ) -> LedgerResult<StateRoot> {
        self.view.compute_root(parent, changes).await
    }

    /// Compute a root using the persisted store.
    pub async fn compute_root_from_store(
        &self,
        parent: ConsensusDigest,
        changes: QmdbChangeSet,
    ) -> LedgerResult<StateRoot> {
        self.view.compute_root_from_store(parent, changes).await
    }

    /// Persist a snapshot and publish an event if a new commit occurs.
    pub async fn persist_snapshot(&self, digest: ConsensusDigest) -> LedgerResult<()> {
        let persisted = self.view.persist_snapshot(digest).await?;
        if persisted {
            self.publish(LedgerEvent::SnapshotPersisted(digest));
        }
        Ok(())
    }

    /// Remove transactions from the mempool.
    pub async fn prune_mempool(&self, txs: &[Tx]) {
        self.view.prune_mempool(txs).await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use alloy_consensus::Header;
    use alloy_primitives::{Address, B256, Bytes, U256};
    use commonware_cryptography::Committable as _;
    use commonware_runtime::{Runner, buffer::PoolRef, tokio};
    use commonware_utils::{NZU16, NZUsize};
    use k256::ecdsa::SigningKey;
    use kora_domain::{Block, ConsensusDigest, Tx, evm::Evm};
    use kora_executor::{BlockContext, BlockExecutor, RevmExecutor};
    use kora_overlay::OverlayState;
    use kora_traits::StateDbRead;

    use super::{LedgerService, LedgerSnapshot, LedgerView};

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
    const CHAIN_ID: u64 = 1337;

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
        BlockContext::new(header, B256::ZERO, prevrandao)
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
                setup.service.parent_snapshot(block1.digest).await.expect("block1 snapshot");
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
            assert!(persisted);
            let state_root =
                setup.ledger.query_state_root(block2.digest).await.expect("state root");
            let qmdb = setup.ledger.inner.lock().await.qmdb.clone();
            let result = qmdb.state().balance(&to).await.expect("balance");
            assert_eq!(result, U256::from(TRANSFER_ONE + TRANSFER_TWO));
            assert_eq!(state_root, block2.block.state_root);
        });
    }

    #[test]
    fn persist_snapshot_duplicate_is_noop() {
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
                "revm-ledger-duplicate",
                vec![(from, U256::from(GENESIS_BALANCE)), (to, U256::ZERO)],
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
                vec![transfer_tx(&from_key, to, TRANSFER_ONE, 0)],
            )
            .await;

            // Act
            let first =
                setup.ledger.persist_snapshot(block.digest).await.expect("persist snapshot");
            assert!(first);

            let second =
                setup.ledger.persist_snapshot(block.digest).await.expect("persist snapshot");

            // Assert
            assert!(!second);
        });
    }

    #[test]
    fn persist_snapshot_merges_overlays() {
        // Tokio runtime required for WrapDatabaseAsync in the QMDB adapter.
        let executor = tokio::Runner::default();
        executor.start(|context| async move {
            // Arrange
            let sender_bytes = [0x11, 0x12, 0x13, 0x14, 0x15];
            let recipient_bytes = [0x21, 0x22, 0x23, 0x24, 0x25];
            let mut sender_keys = Vec::new();
            let mut recipients = Vec::new();
            let mut genesis_alloc = Vec::new();
            for (sender_byte, recipient_byte) in sender_bytes.iter().zip(recipient_bytes.iter()) {
                let recipient_key = key_from_byte(*recipient_byte);
                let recipient = Evm::address_from_key(&recipient_key);
                recipients.push(recipient);
                genesis_alloc.push((recipient, U256::ZERO));
                let key = key_from_byte(*sender_byte);
                let addr = Evm::address_from_key(&key);
                sender_keys.push(key);
                genesis_alloc.push((addr, U256::from(GENESIS_BALANCE)));
            }
            let setup = setup_ledger(context, "revm-ledger-overlay", genesis_alloc).await;
            let parent_snapshot = setup
                .service
                .parent_snapshot(setup.genesis_digest)
                .await
                .expect("genesis snapshot");
            let txs: Vec<Tx> = sender_keys
                .iter()
                .zip(recipients.iter().copied())
                .map(|(key, recipient)| transfer_tx(key, recipient, TRANSFER_DUPLICATE, 0))
                .collect();
            let block = build_block_snapshot(
                &setup.service,
                &setup.genesis,
                parent_snapshot,
                HEIGHT_ONE,
                txs,
            )
            .await;

            // Act
            let persisted = setup.ledger.persist_snapshot(block.digest).await.expect("persist");
            assert!(persisted);

            // Assert
            let qmdb = setup.ledger.inner.lock().await.qmdb.clone();
            for recipient in recipients {
                let result = qmdb.state().balance(&recipient).await.expect("balance");
                assert_eq!(result, U256::from(TRANSFER_DUPLICATE));
            }
        });
    }

    #[test]
    fn persist_snapshot_unrelated_merges() {
        // Tokio runtime required for WrapDatabaseAsync in the QMDB adapter.
        let executor = tokio::Runner::default();
        executor.start(|context| async move {
            // Arrange
            let from_key_a = key_from_byte(FROM_BYTE_A);
            let to_key_a = key_from_byte(TO_BYTE_A);
            let from_a = Evm::address_from_key(&from_key_a);
            let to_a = Evm::address_from_key(&to_key_a);
            let from_key_b = key_from_byte(FROM_BYTE_B);
            let to_key_b = key_from_byte(TO_BYTE_B);
            let from_b = Evm::address_from_key(&from_key_b);
            let to_b = Evm::address_from_key(&to_key_b);
            let setup = setup_ledger(
                context,
                "revm-ledger-unrelated",
                vec![
                    (from_a, U256::from(GENESIS_BALANCE)),
                    (to_a, U256::ZERO),
                    (from_b, U256::from(DUPLICATE_BALANCE)),
                    (to_b, U256::ZERO),
                ],
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
                vec![transfer_tx(&from_key_a, to_a, TRANSFER_ONE, 0)],
            )
            .await;
            let parent_snapshot = setup
                .service
                .parent_snapshot(setup.genesis_digest)
                .await
                .expect("genesis snapshot");
            let block2 = build_block_snapshot(
                &setup.service,
                &setup.genesis,
                parent_snapshot,
                HEIGHT_ONE,
                vec![transfer_tx(&from_key_b, to_b, TRANSFER_DUPLICATE, 0)],
            )
            .await;

            // Act
            let persisted_1 =
                setup.ledger.persist_snapshot(block1.digest).await.expect("persist snapshot");
            let persisted_2 =
                setup.ledger.persist_snapshot(block2.digest).await.expect("persist snapshot");

            // Assert
            assert!(persisted_1);
            assert!(persisted_2);
            let qmdb = setup.ledger.inner.lock().await.qmdb.clone();
            assert_eq!(
                qmdb.state().balance(&to_a).await.expect("balance"),
                U256::from(TRANSFER_ONE)
            );
            assert_eq!(
                qmdb.state().balance(&to_b).await.expect("balance"),
                U256::from(TRANSFER_DUPLICATE)
            );
        });
    }

    #[test]
    fn persist_snapshot_updates_snapshot_state() {
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
                "revm-ledger-updates",
                vec![(from, U256::from(GENESIS_BALANCE)), (to, U256::ZERO)],
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
                vec![transfer_tx(&from_key, to, TRANSFER_ONE, 0)],
            )
            .await;

            // Act
            let persisted = setup.ledger.persist_snapshot(block.digest).await.expect("persist");

            // Assert
            assert!(persisted);
            let state_root = setup.ledger.query_state_root(block.digest).await.expect("state root");
            assert_eq!(state_root, block.block.state_root);
        });
    }
}
