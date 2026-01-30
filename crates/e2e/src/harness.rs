//! Test harness for running multi-node consensus tests.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use alloy_consensus::Header;
use alloy_primitives::{Address, B256, U256};
use anyhow::Context as _;
use commonware_consensus::{
    Reporter, Reporters,
    application::marshaled::Marshaled,
    simplex::{self, elector::Random, types::Finalization},
    types::{Epoch, FixedEpocher, ViewDelta},
};
use commonware_cryptography::{bls12381::primitives::variant::MinSig, ed25519};
use commonware_p2p::{Manager as _, simulated};
use commonware_parallel::Sequential;
use commonware_runtime::{Clock, Metrics, Runner as _, Spawner, buffer::PoolRef, tokio};
use commonware_utils::{NZU64, NZUsize, TryCollect as _, ordered::Set};
use futures::{StreamExt as _, channel::mpsc};
use kora_crypto::{ThresholdScheme, threshold_schemes};
use kora_domain::{
    Block, BlockCfg, ConsensusDigest, FinalizationEvent, LedgerEvent, PublicKey, StateRoot, TxCfg,
};
use kora_executor::{BlockContext, RevmExecutor};
use kora_ledger::{LedgerService, LedgerView};
use kora_marshal::{ArchiveInitializer, BroadcastInitializer, PeerInitializer};
use kora_reporters::{BlockContextProvider, FinalizedReporter, SeedReporter};
use kora_simplex::{DEFAULT_MAILBOX_SIZE as MAILBOX_SIZE, DefaultPool, DefaultQuota};
use kora_transport_sim::{SimContext, SimControl, SimLinkConfig, register_node_channels};
use thiserror::Error;
use tracing::{debug, info, trace};

use crate::{TestConfig, TestNode, TestSetup};

const MAX_MSG_SIZE: usize = 1024 * 1024;
const BLOCK_CODEC_MAX_TXS: usize = 64;
const BLOCK_CODEC_MAX_TX_BYTES: usize = 1024;
const EPOCH_LENGTH: u64 = u64::MAX;

type Peer = PublicKey;
type CertArchive = Finalization<ThresholdScheme, ConsensusDigest>;

/// Errors from test harness execution.
#[derive(Debug, Error)]
pub enum HarnessError {
    /// Failed to initialize threshold schemes.
    #[error("threshold scheme init failed: {0}")]
    SchemeInit(#[from] anyhow::Error),
    /// Finalization timeout.
    #[error("finalization timeout: expected {expected} blocks, got {actual}")]
    FinalizationTimeout {
        /// Expected block count.
        expected: u64,
        /// Actual block count.
        actual: u64,
    },
    /// State divergence between nodes.
    #[error("state divergence at block {digest:?}: {message}")]
    StateDivergence {
        /// The block digest where divergence occurred.
        digest: ConsensusDigest,
        /// Description of the divergence.
        message: String,
    },
    /// Missing expected state.
    #[error("missing state: {0}")]
    MissingState(String),
    /// Balance mismatch.
    #[error("balance mismatch for {address}: expected {expected}, got {actual}")]
    BalanceMismatch {
        /// The address with the mismatch.
        address: Address,
        /// Expected balance.
        expected: U256,
        /// Actual balance.
        actual: U256,
    },
}

/// Outcome of a successful test run.
#[derive(Debug, Clone)]
pub struct TestOutcome {
    /// Final block digest that all nodes agree on.
    pub finalized_head: ConsensusDigest,
    /// State root at the finalized head.
    pub state_root: StateRoot,
    /// Seed (prevrandao) at the finalized head.
    pub seed: B256,
    /// Number of blocks finalized.
    pub blocks_finalized: u64,
    /// Per-node finalization counts.
    pub node_finalization_counts: Vec<u64>,
}

/// Test harness for running e2e consensus tests.
#[derive(Debug)]
pub struct TestHarness;

impl TestHarness {
    /// Run a test with the given configuration and setup.
    pub fn run(config: TestConfig, setup: TestSetup) -> Result<TestOutcome, HarnessError> {
        let executor = tokio::Runner::default();
        executor.start(|context| async move { Self::run_inner(context, config, setup).await })
    }

    async fn run_inner(
        context: tokio::Context,
        config: TestConfig,
        setup: TestSetup,
    ) -> Result<TestOutcome, HarnessError> {
        // Generate a unique partition prefix for this test run to avoid conflicts.
        // Use a combination of timestamp, seed, and atomic counter to ensure uniqueness.
        use std::sync::atomic::{AtomicU64, Ordering};
        static RUN_COUNTER: AtomicU64 = AtomicU64::new(0);
        let counter = RUN_COUNTER.fetch_add(1, Ordering::SeqCst);
        let run_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let partition_prefix = format!("e2e-{run_id}-{counter}-s{}", config.seed);

        info!(
            validators = config.validators,
            threshold = config.threshold,
            max_blocks = config.max_blocks,
            %partition_prefix,
            "Starting e2e test"
        );

        // Generate threshold signing schemes
        let (participants_vec, schemes) = threshold_schemes(config.seed, config.validators)?;
        let participants_set = participants_set(&participants_vec)?;

        // Start simulated network
        let mut sim_control = start_network(&context, participants_set).await;
        sim_control
            .connect_all(
                &participants_vec,
                SimLinkConfig {
                    latency: config.link.latency,
                    jitter: config.link.jitter,
                    success_rate: config.link.success_rate,
                },
            )
            .await
            .context("connect_all")?;
        let sim_control = Arc::new(Mutex::new(sim_control));

        // Start all nodes
        let bootstrap = setup.to_bootstrap();
        let (nodes, mut finalized_rx) = start_all_nodes(
            &context,
            &sim_control,
            &participants_vec,
            &schemes,
            &bootstrap,
            config.chain_id,
            config.gas_limit,
            &partition_prefix,
        )
        .await?;

        // Wait for finalization
        let (head, node_counts) = wait_for_finalized_head(
            &mut finalized_rx,
            config.validators,
            config.max_blocks,
            config.timeout,
        )
        .await?;

        info!(?head, "All nodes reached finalization");

        // Verify state convergence
        let (state_root, seed) = verify_state_convergence(&nodes, head).await?;

        // Verify expected balances
        verify_expected_balances(&nodes[0], head, &setup.expected_balances).await?;

        Ok(TestOutcome {
            finalized_head: head,
            state_root,
            seed,
            blocks_finalized: config.max_blocks,
            node_finalization_counts: node_counts,
        })
    }
}

fn participants_set(
    participants: &[ed25519::PublicKey],
) -> anyhow::Result<Set<ed25519::PublicKey>> {
    participants
        .iter()
        .cloned()
        .try_collect()
        .map_err(|_| anyhow::anyhow!("participant public keys are not unique"))
}

async fn start_network(
    context: &tokio::Context,
    participants: Set<ed25519::PublicKey>,
) -> SimControl<ed25519::PublicKey> {
    let (network, oracle) = simulated::Network::new(
        SimContext::new(context.with_label("network")),
        simulated::Config {
            max_size: MAX_MSG_SIZE as u32,
            disconnect_on_block: true,
            tracked_peer_sets: None,
        },
    );
    network.start();

    let control = SimControl::new(oracle);
    control.manager().update(0, participants).await;
    control
}

const fn block_codec_cfg() -> BlockCfg {
    BlockCfg { max_txs: BLOCK_CODEC_MAX_TXS, tx: TxCfg { max_tx_bytes: BLOCK_CODEC_MAX_TX_BYTES } }
}

#[derive(Clone, Debug)]
struct TestContextProvider {
    gas_limit: u64,
}

impl BlockContextProvider for TestContextProvider {
    fn context(&self, block: &Block) -> BlockContext {
        let header = Header {
            number: block.height,
            timestamp: block.height,
            gas_limit: self.gas_limit,
            beneficiary: Address::ZERO,
            base_fee_per_gas: Some(0),
            ..Default::default()
        };
        BlockContext::new(header, B256::ZERO, block.prevrandao)
    }
}

async fn start_all_nodes(
    context: &tokio::Context,
    sim_control: &Arc<Mutex<SimControl<ed25519::PublicKey>>>,
    participants: &[ed25519::PublicKey],
    schemes: &[ThresholdScheme],
    bootstrap: &kora_domain::BootstrapConfig,
    chain_id: u64,
    gas_limit: u64,
    partition_prefix: &str,
) -> anyhow::Result<(Vec<TestNode>, mpsc::UnboundedReceiver<FinalizationEvent>)> {
    let (finalized_tx, finalized_rx) = mpsc::unbounded::<FinalizationEvent>();
    let mut nodes = Vec::with_capacity(participants.len());

    let manager = {
        let control = sim_control.lock().map_err(|_| anyhow::anyhow!("lock poisoned"))?;
        control.manager()
    };

    for (i, pk) in participants.iter().cloned().enumerate() {
        let node = start_single_node(
            context,
            sim_control,
            manager.clone(),
            i,
            pk,
            schemes[i].clone(),
            bootstrap.clone(),
            finalized_tx.clone(),
            chain_id,
            gas_limit,
            partition_prefix,
        )
        .await?;
        nodes.push(node);
    }

    Ok((nodes, finalized_rx))
}

#[allow(clippy::too_many_arguments)]
async fn start_single_node(
    context: &tokio::Context,
    sim_control: &Arc<Mutex<SimControl<ed25519::PublicKey>>>,
    manager: simulated::Manager<Peer, SimContext>,
    index: usize,
    public_key: Peer,
    scheme: ThresholdScheme,
    bootstrap: kora_domain::BootstrapConfig,
    finalized_tx: mpsc::UnboundedSender<FinalizationEvent>,
    chain_id: u64,
    gas_limit: u64,
    partition_prefix: &str,
) -> anyhow::Result<TestNode> {
    let quota = DefaultQuota::init();
    let buffer_pool = DefaultPool::init();
    let block_cfg = block_codec_cfg();

    let mut control = {
        let sim = sim_control.lock().map_err(|_| anyhow::anyhow!("lock poisoned"))?;
        sim.peer_control(public_key.clone())
    };
    let blocker = control.clone();

    let channels = register_node_channels(&mut control, quota)
        .await
        .map_err(|e| anyhow::anyhow!("channel registration failed: {e}"))?;

    // Initialize ledger
    let state = LedgerView::init(
        context.with_label(&format!("state_{index}")),
        buffer_pool.clone(),
        format!("{partition_prefix}-qmdb-{index}"),
        bootstrap.genesis_alloc.clone(),
    )
    .await
    .context("init qmdb")?;

    let ledger = LedgerService::new(state.clone());
    spawn_ledger_observers(ledger.clone(), context.clone(), index, finalized_tx);
    let test_node = TestNode::new(index, ledger.clone());

    // Create application
    let app = TestApplication::<ThresholdScheme>::new(block_cfg.max_txs, state.clone());

    // Create finalized reporter
    let executor = RevmExecutor::new(chain_id);
    let context_provider = TestContextProvider { gas_limit };
    let finalized_reporter =
        FinalizedReporter::new(ledger.clone(), context.clone(), executor, context_provider);

    // Start marshal
    let marshal_mailbox = start_marshal(
        context,
        index,
        public_key.clone(),
        control.clone(),
        manager,
        scheme.clone(),
        buffer_pool.clone(),
        block_cfg,
        channels.marshal.blocks,
        channels.marshal.backfill,
        finalized_reporter,
        partition_prefix,
    )
    .await?;

    // Create marshaled application
    let epocher = FixedEpocher::new(NZU64!(EPOCH_LENGTH));
    let marshaled = Marshaled::new(
        context.with_label(&format!("marshaled_{index}")),
        app,
        marshal_mailbox.clone(),
        epocher,
    );

    // Setup reporters
    let seed_reporter = SeedReporter::<MinSig>::new(ledger.clone());
    let reporter = Reporters::from((seed_reporter, marshal_mailbox.clone()));

    // Submit bootstrap transactions
    for tx in &bootstrap.bootstrap_txs {
        let _ = ledger.submit_tx(tx.clone()).await;
    }

    // Start consensus engine
    let engine = simplex::Engine::new(
        context.with_label(&format!("engine_{index}")),
        simplex::Config {
            scheme,
            elector: Random,
            blocker,
            automaton: marshaled.clone(),
            relay: marshaled,
            reporter,
            strategy: Sequential,
            partition: format!("{partition_prefix}-{index}"),
            mailbox_size: MAILBOX_SIZE,
            epoch: Epoch::zero(),
            replay_buffer: NZUsize!(1024 * 1024),
            write_buffer: NZUsize!(1024 * 1024),
            leader_timeout: Duration::from_secs(1),
            notarization_timeout: Duration::from_secs(2),
            nullify_retry: Duration::from_secs(5),
            fetch_timeout: Duration::from_secs(1),
            activity_timeout: ViewDelta::new(20),
            skip_timeout: ViewDelta::new(10),
            fetch_concurrent: 8,
            buffer_pool,
        },
    );
    engine.start(channels.simplex.votes, channels.simplex.certs, channels.simplex.resolver);

    debug!(index, "Node started");
    Ok(test_node)
}

fn spawn_ledger_observers<S: Spawner>(
    service: LedgerService,
    spawner: S,
    node_index: usize,
    finalized_tx: mpsc::UnboundedSender<FinalizationEvent>,
) {
    let mut receiver = service.subscribe();
    spawner.shared(true).spawn(move |_| async move {
        while let Some(event) = receiver.next().await {
            match event {
                LedgerEvent::TransactionSubmitted(id) => {
                    trace!(node = node_index, tx=?id, "tx submitted");
                }
                LedgerEvent::SeedUpdated(digest, seed) => {
                    trace!(node = node_index, ?digest, ?seed, "seed updated");
                }
                LedgerEvent::SnapshotPersisted(digest) => {
                    trace!(node = node_index, ?digest, "snapshot persisted");
                    let _ = finalized_tx.unbounded_send((node_index as u32, digest));
                }
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
async fn start_marshal<M, R>(
    context: &tokio::Context,
    index: usize,
    public_key: Peer,
    control: simulated::Control<Peer, SimContext>,
    manager: M,
    scheme: ThresholdScheme,
    buffer_pool: PoolRef,
    block_codec_config: BlockCfg,
    blocks: (simulated::Sender<Peer, SimContext>, simulated::Receiver<Peer>),
    backfill: (simulated::Sender<Peer, SimContext>, simulated::Receiver<Peer>),
    application: R,
    partition_prefix: &str,
) -> anyhow::Result<commonware_consensus::marshal::Mailbox<ThresholdScheme, Block>>
where
    M: commonware_p2p::Manager<PublicKey = Peer>,
    R: Reporter<Activity = commonware_consensus::marshal::Update<Block>> + Send + 'static,
{
    use std::sync::Arc;

    use commonware_cryptography::certificate::Scheme as _;
    use commonware_utils::acknowledgement::Exact;

    let ctx = context.with_label(&format!("marshal_{index}"));
    let marshal_partition = format!("{partition_prefix}-marshal-{index}");

    #[derive(Clone)]
    struct ConstantSchemeProvider(Arc<ThresholdScheme>);

    impl commonware_cryptography::certificate::Provider for ConstantSchemeProvider {
        type Scope = Epoch;
        type Scheme = ThresholdScheme;

        fn scoped(&self, _epoch: Epoch) -> Option<Arc<Self::Scheme>> {
            Some(self.0.clone())
        }

        fn all(&self) -> Option<Arc<Self::Scheme>> {
            Some(self.0.clone())
        }
    }

    let scheme_provider = ConstantSchemeProvider(Arc::new(scheme));

    let resolver = PeerInitializer::init::<_, _, _, Block, _, _, _>(
        &ctx,
        public_key.clone(),
        manager,
        control,
        backfill,
    );

    let (broadcast_engine, buffer) = BroadcastInitializer::init::<_, PublicKey, Block>(
        ctx.with_label("broadcast"),
        public_key,
        block_codec_config,
    );
    broadcast_engine.start(blocks);

    ThresholdScheme::certificate_codec_config_unbounded();
    let finalizations_by_height = ArchiveInitializer::init::<_, ConsensusDigest, CertArchive>(
        ctx.with_label("finalizations_by_height"),
        format!("{marshal_partition}-finalizations-by-height"),
        (),
    )
    .await
    .context("init finalizations archive")?;

    let finalized_blocks = ArchiveInitializer::init::<_, ConsensusDigest, Block>(
        ctx.with_label("finalized_blocks"),
        format!("{marshal_partition}-finalized-blocks"),
        block_codec_config,
    )
    .await
    .context("init blocks archive")?;

    let (actor, mailbox, _last_processed_height) =
        kora_marshal::ActorInitializer::init_with_partition::<_, Block, _, _, _, Exact>(
            ctx.clone(),
            finalizations_by_height,
            finalized_blocks,
            scheme_provider,
            buffer_pool,
            block_codec_config,
            format!("{marshal_partition}-actor"),
        )
        .await;
    actor.start(application, buffer, resolver);

    Ok(mailbox)
}

async fn wait_for_finalized_head(
    finalized_rx: &mut mpsc::UnboundedReceiver<FinalizationEvent>,
    nodes: usize,
    target_blocks: u64,
    timeout: Duration,
) -> Result<(ConsensusDigest, Vec<u64>), HarnessError> {
    use ::tokio::time::timeout as tokio_timeout;

    let mut counts = vec![0u64; nodes];
    let mut nth = vec![None; nodes];

    let result = tokio_timeout(timeout, async {
        while nth.iter().any(Option::is_none) {
            let Some((node, digest)) = finalized_rx.next().await else {
                break;
            };
            let idx = node as usize;
            if idx >= nodes || nth[idx].is_some() {
                continue;
            }
            counts[idx] += 1;
            if counts[idx] == target_blocks {
                nth[idx] = Some(digest);
                info!(node = idx, ?digest, blocks = counts[idx], "Node reached target");
            }
        }
    })
    .await;

    if result.is_err() {
        let actual = counts.iter().min().copied().unwrap_or(0);
        return Err(HarnessError::FinalizationTimeout { expected: target_blocks, actual });
    }

    let head = nth
        .first()
        .and_then(|d| *d)
        .ok_or_else(|| HarnessError::MissingState("no finalization received".to_string()))?;

    for (i, d) in nth.iter().enumerate() {
        let Some(d) = d else {
            return Err(HarnessError::StateDivergence {
                digest: head,
                message: format!("node {i} missing finalization"),
            });
        };
        if *d != head {
            return Err(HarnessError::StateDivergence {
                digest: head,
                message: format!("node {i} has different head"),
            });
        }
    }

    Ok((head, counts))
}

async fn verify_state_convergence(
    nodes: &[TestNode],
    head: ConsensusDigest,
) -> Result<(StateRoot, B256), HarnessError> {
    let mut state_root = None;
    let mut seed = None;

    for node in nodes.iter() {
        let root = node.query_state_root(head).await.ok_or_else(|| {
            HarnessError::MissingState(format!("node {} missing state root", node.index))
        })?;

        state_root = match state_root {
            None => Some(root),
            Some(prev) if prev == root => Some(prev),
            Some(prev) => {
                return Err(HarnessError::StateDivergence {
                    digest: head,
                    message: format!("state root mismatch: {:?} vs {:?}", prev, root),
                });
            }
        };

        let node_seed = node.query_seed(head).await.ok_or_else(|| {
            HarnessError::MissingState(format!("node {} missing seed", node.index))
        })?;

        seed = match seed {
            None => Some(node_seed),
            Some(prev) if prev == node_seed => Some(prev),
            Some(prev) => {
                return Err(HarnessError::StateDivergence {
                    digest: head,
                    message: format!("seed mismatch: {:?} vs {:?}", prev, node_seed),
                });
            }
        };
    }

    let state_root =
        state_root.ok_or_else(|| HarnessError::MissingState("no state root".to_string()))?;
    let seed = seed.ok_or_else(|| HarnessError::MissingState("no seed".to_string()))?;

    Ok((state_root, seed))
}

async fn verify_expected_balances(
    node: &TestNode,
    head: ConsensusDigest,
    expected: &[(Address, U256)],
) -> Result<(), HarnessError> {
    for (address, expected_balance) in expected {
        let actual = node
            .query_balance(head, *address)
            .await
            .ok_or_else(|| HarnessError::MissingState(format!("missing balance for {address}")))?;

        if actual != *expected_balance {
            return Err(HarnessError::BalanceMismatch {
                address: *address,
                expected: *expected_balance,
                actual,
            });
        }
    }
    Ok(())
}

// Import the test application from the revm example pattern
use std::collections::BTreeSet;

use alloy_primitives::Bytes;
use commonware_consensus::{
    Application, Block as _, VerifyingApplication, marshal::ingress::mailbox::AncestorStream,
    simplex::types::Context,
};
use commonware_cryptography::{Committable as _, certificate::Scheme as CertScheme};
use kora_consensus::{
    BlockExecution, Mempool as _, SnapshotStore, components::InMemorySnapshotStore,
};
use kora_executor::BlockExecutor;
use kora_overlay::OverlayState;
use kora_qmdb_ledger::QmdbState;
use rand::Rng;

#[derive(Clone)]
struct TestApplication<S> {
    ledger: LedgerView,
    executor: RevmExecutor,
    max_txs: usize,
    gas_limit: u64,
    _scheme: std::marker::PhantomData<S>,
}

impl<S> std::fmt::Debug for TestApplication<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestApplication").finish_non_exhaustive()
    }
}

impl<S> TestApplication<S> {
    const fn new(max_txs: usize, ledger: LedgerView) -> Self {
        Self {
            ledger,
            executor: RevmExecutor::new(1337),
            max_txs,
            gas_limit: 30_000_000,
            _scheme: std::marker::PhantomData,
        }
    }

    fn block_context(&self, height: u64, prevrandao: B256) -> BlockContext {
        let header = Header {
            number: height,
            timestamp: height,
            gas_limit: self.gas_limit,
            beneficiary: Address::ZERO,
            base_fee_per_gas: Some(0),
            ..Default::default()
        };
        BlockContext::new(header, B256::ZERO, prevrandao)
    }

    async fn get_prevrandao(&self, parent_digest: ConsensusDigest) -> B256 {
        self.ledger.seed_for_parent(parent_digest).await.unwrap_or(B256::ZERO)
    }

    async fn build_block(&self, parent: &Block) -> Option<Block> {
        let parent_digest = parent.commitment();
        let parent_snapshot = self.ledger.parent_snapshot(parent_digest).await?;

        let (_, mempool, snapshots) = self.ledger.proposal_components().await;
        let excluded = self.collect_pending_tx_ids(&snapshots, parent_digest);
        let txs = mempool.build(self.max_txs, &excluded);

        let prevrandao = self.get_prevrandao(parent_digest).await;
        let height = parent.height + 1;
        let context = self.block_context(height, prevrandao);
        let txs_bytes: Vec<Bytes> = txs.iter().map(|tx| tx.bytes.clone()).collect();

        let outcome = self.executor.execute(&parent_snapshot.state, &context, &txs_bytes).ok()?;

        let state_root = self
            .ledger
            .compute_root_from_store(parent_digest, outcome.changes.clone())
            .await
            .ok()?;

        let block = Block { parent: parent.id(), height, prevrandao, state_root, txs };

        let merged_changes = parent_snapshot.state.merge_changes(outcome.changes.clone());
        let next_state = OverlayState::new(parent_snapshot.state.base(), merged_changes);
        let block_digest = block.commitment();

        self.ledger
            .insert_snapshot(
                block_digest,
                parent_digest,
                next_state,
                state_root,
                outcome.changes,
                &block.txs,
            )
            .await;

        Some(block)
    }

    async fn verify_block(&self, block: &Block) -> bool {
        let digest = block.commitment();
        let parent_digest = block.parent();

        if self.ledger.query_state_root(digest).await.is_some() {
            return true;
        }

        let Some(parent_snapshot) = self.ledger.parent_snapshot(parent_digest).await else {
            return false;
        };

        let context = self.block_context(block.height, block.prevrandao);
        let execution =
            match BlockExecution::execute(&parent_snapshot, &self.executor, &context, &block.txs)
                .await
            {
                Ok(result) => result,
                Err(_) => return false,
            };

        let state_root = match self
            .ledger
            .compute_root_from_store(parent_digest, execution.outcome.changes.clone())
            .await
        {
            Ok(root) => root,
            Err(_) => return false,
        };

        if state_root != block.state_root {
            return false;
        }

        let merged_changes = parent_snapshot.state.merge_changes(execution.outcome.changes.clone());
        let next_state = OverlayState::new(parent_snapshot.state.base(), merged_changes);

        self.ledger
            .insert_snapshot(
                digest,
                parent_digest,
                next_state,
                state_root,
                execution.outcome.changes,
                &block.txs,
            )
            .await;

        true
    }

    fn collect_pending_tx_ids(
        &self,
        snapshots: &InMemorySnapshotStore<OverlayState<QmdbState>>,
        from: ConsensusDigest,
    ) -> BTreeSet<kora_consensus::TxId> {
        let mut excluded = BTreeSet::new();
        let mut current = Some(from);

        while let Some(digest) = current {
            if snapshots.is_persisted(&digest) {
                break;
            }
            let Some(snapshot) = snapshots.get(&digest) else {
                break;
            };
            excluded.extend(snapshot.tx_ids.iter().copied());
            current = snapshot.parent;
        }

        excluded
    }
}

impl<Env, S> Application<Env> for TestApplication<S>
where
    Env: Rng + Spawner + Metrics + Clock,
    S: CertScheme + Send + Sync + 'static,
{
    type SigningScheme = S;
    type Context = Context<ConsensusDigest, S::PublicKey>;
    type Block = Block;

    fn genesis(&mut self) -> impl std::future::Future<Output = Self::Block> + Send {
        async move { self.ledger.genesis_block() }
    }

    fn propose(
        &mut self,
        _context: (Env, Self::Context),
        mut ancestry: AncestorStream<Self::SigningScheme, Self::Block>,
    ) -> impl std::future::Future<Output = Option<Self::Block>> + Send {
        async move {
            let parent = ancestry.next().await?;
            self.build_block(&parent).await
        }
    }
}

impl<Env, S> VerifyingApplication<Env> for TestApplication<S>
where
    Env: Rng + Spawner + Metrics + Clock,
    S: CertScheme + Send + Sync + 'static,
{
    fn verify(
        &mut self,
        _context: (Env, Self::Context),
        mut ancestry: AncestorStream<Self::SigningScheme, Self::Block>,
    ) -> impl std::future::Future<Output = bool> + Send {
        async move {
            let mut blocks_to_verify = Vec::new();
            while let Some(block) = ancestry.next().await {
                let digest = block.commitment();
                if self.ledger.query_state_root(digest).await.is_some() {
                    break;
                }
                blocks_to_verify.push(block);
            }

            for block in blocks_to_verify.into_iter().rev() {
                if !self.verify_block(&block).await {
                    return false;
                }
            }

            true
        }
    }
}
