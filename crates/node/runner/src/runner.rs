use std::{sync::Arc, time::Duration};

use alloy_consensus::Header;
use alloy_primitives::{Address, B256};
use anyhow::Context as _;
use commonware_consensus::{
    Reporters,
    application::marshaled::Marshaled,
    simplex::{self, elector::Random, types::Finalization},
    types::{Epoch, FixedEpocher, ViewDelta},
};
use commonware_cryptography::{bls12381::primitives::variant::MinSig, ed25519};
use commonware_p2p::Manager;
use commonware_parallel::Sequential;
use commonware_runtime::{Metrics as _, Spawner, buffer::PoolRef, tokio};
use commonware_utils::{NZU64, NZUsize, acknowledgement::Exact};
use futures::StreamExt;
use kora_domain::{Block, BlockCfg, BootstrapConfig, ConsensusDigest, LedgerEvent, TxCfg};
use kora_executor::{BlockContext, RevmExecutor};
use kora_ledger::{LedgerService, LedgerView};
use kora_marshal::{ArchiveInitializer, BroadcastInitializer, PeerInitializer};
use kora_reporters::{BlockContextProvider, FinalizedReporter, NodeStateReporter, SeedReporter};
use kora_service::{NodeRunContext, NodeRunner};
use kora_simplex::{DEFAULT_MAILBOX_SIZE as MAILBOX_SIZE, DefaultPool};
use kora_transport::NetworkTransport;
use tracing::{debug, info, trace};

use crate::{RevmApplication, RunnerError, scheme::ThresholdScheme};

const BLOCK_CODEC_MAX_TXS: usize = 64;
const BLOCK_CODEC_MAX_TX_BYTES: usize = 1024;
const EPOCH_LENGTH: u64 = u64::MAX;
const PARTITION_PREFIX: &str = "kora";

type Peer = ed25519::PublicKey;
type CertArchive = Finalization<ThresholdScheme, ConsensusDigest>;
type MarshalMailbox = commonware_consensus::marshal::Mailbox<ThresholdScheme, Block>;
type NodeStateRptr = NodeStateReporter<ThresholdScheme>;

fn default_buffer_pool() -> PoolRef {
    DefaultPool::init()
}

const fn block_codec_cfg() -> BlockCfg {
    BlockCfg { max_txs: BLOCK_CODEC_MAX_TXS, tx: TxCfg { max_tx_bytes: BLOCK_CODEC_MAX_TX_BYTES } }
}

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

impl From<ThresholdScheme> for ConstantSchemeProvider {
    fn from(scheme: ThresholdScheme) -> Self {
        Self(Arc::new(scheme))
    }
}

#[derive(Clone, Debug)]
struct RevmContextProvider {
    gas_limit: u64,
}

impl BlockContextProvider for RevmContextProvider {
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

fn spawn_ledger_observers<S: Spawner>(service: LedgerService, spawner: S) {
    let mut receiver = service.subscribe();
    spawner.shared(true).spawn(move |_| async move {
        while let Some(event) = receiver.next().await {
            match event {
                LedgerEvent::TransactionSubmitted(id) => {
                    trace!(tx=?id, "mempool accepted transaction");
                }
                LedgerEvent::SeedUpdated(digest, seed) => {
                    debug!(digest=?digest, seed=?seed, "seed cache refreshed");
                }
                LedgerEvent::SnapshotPersisted(digest) => {
                    trace!(?digest, "snapshot persisted");
                }
            }
        }
    });
}

/// Production validator node runner.
#[derive(Clone, Debug)]
pub struct ProductionRunner {
    /// Threshold signing scheme.
    pub scheme: ThresholdScheme,
    /// Chain ID.
    pub chain_id: u64,
    /// Gas limit per block.
    pub gas_limit: u64,
    /// Bootstrap configuration.
    pub bootstrap: BootstrapConfig,
    /// Storage partition prefix.
    pub partition_prefix: String,
    /// Optional RPC configuration (state, bind address).
    pub rpc_config: Option<(kora_rpc::NodeState, std::net::SocketAddr)>,
}

impl ProductionRunner {
    /// Create a new production runner.
    pub fn new(
        scheme: ThresholdScheme,
        chain_id: u64,
        gas_limit: u64,
        bootstrap: BootstrapConfig,
    ) -> Self {
        Self {
            scheme,
            chain_id,
            gas_limit,
            bootstrap,
            partition_prefix: PARTITION_PREFIX.to_string(),
            rpc_config: None,
        }
    }

    /// Configure RPC server.
    pub fn with_rpc(mut self, state: kora_rpc::NodeState, addr: std::net::SocketAddr) -> Self {
        self.rpc_config = Some((state, addr));
        self
    }
}

impl ProductionRunner {
    /// Run the validator as a standalone process.
    pub fn run_standalone(self, config: kora_config::NodeConfig) -> Result<(), RunnerError> {
        use commonware_runtime::Runner;
        use kora_transport::NetworkConfigExt;

        let rpc_config = self.rpc_config.clone();

        let executor = tokio::Runner::default();
        executor.start(|context| async move {
            // Start RPC server if configured
            if let Some((state, addr)) = rpc_config {
                let rpc = kora_rpc::RpcServer::new(state, addr);
                drop(rpc.start());
            }

            let validator_key = config
                .validator_key()
                .map_err(|e| anyhow::anyhow!("failed to load validator key: {}", e))?;

            let transport = config
                .network
                .build_local_transport(validator_key, context.clone())
                .map_err(|e| anyhow::anyhow!("failed to build transport: {}", e))?;

            let ctx =
                kora_service::NodeRunContext::new(context, std::sync::Arc::new(config), transport);

            let _ledger = self.run(ctx).await?;

            futures::future::pending::<()>().await;
            Ok::<(), RunnerError>(())
        })
    }
}

impl NodeRunner for ProductionRunner {
    type Transport = NetworkTransport<Peer, tokio::Context>;
    type Handle = LedgerService;
    type Error = RunnerError;

    async fn run(&self, ctx: NodeRunContext<Self::Transport>) -> Result<Self::Handle, Self::Error> {
        let (context, config, mut transport) = ctx.into_parts();

        info!(chain_id = self.chain_id, "Starting production validator");

        let validators = self.scheme.participants().clone();
        transport.oracle.update(0, validators).await;
        info!(count = self.scheme.participants().len(), "Registered validators with oracle");

        let buffer_pool = default_buffer_pool();
        let block_cfg = block_codec_cfg();

        let state = LedgerView::init(
            context.with_label("state"),
            buffer_pool.clone(),
            format!("{}-qmdb", self.partition_prefix),
            self.bootstrap.genesis_alloc.clone(),
        )
        .await
        .context("init qmdb")?;

        let ledger = LedgerService::new(state.clone());
        spawn_ledger_observers(ledger.clone(), context.clone());

        let validator_key = config
            .validator_key()
            .map_err(|e| anyhow::anyhow!("failed to load validator key: {}", e))?;
        let my_pk = commonware_cryptography::Signer::public_key(&validator_key);

        let executor = RevmExecutor::new(self.chain_id);
        let context_provider = RevmContextProvider { gas_limit: self.gas_limit };
        let finalized_reporter =
            FinalizedReporter::new(ledger.clone(), context.clone(), executor, context_provider);

        let scheme_provider = ConstantSchemeProvider::from(self.scheme.clone());

        let resolver = PeerInitializer::init::<_, _, _, Block, _, _, _>(
            &context.with_label("resolver"),
            my_pk.clone(),
            transport.oracle.clone(),
            transport.oracle.clone(),
            transport.marshal.backfill,
        );

        let (broadcast_engine, buffer) = BroadcastInitializer::init::<_, Peer, Block>(
            context.with_label("broadcast"),
            my_pk.clone(),
            block_cfg,
        );
        broadcast_engine.start(transport.marshal.blocks);

        let partition_prefix = &self.partition_prefix;
        <ThresholdScheme as commonware_cryptography::certificate::Scheme>::certificate_codec_config_unbounded();
        let finalizations_by_height = ArchiveInitializer::init::<_, ConsensusDigest, CertArchive>(
            context.with_label("finalizations_by_height"),
            format!("{partition_prefix}-finalizations-by-height"),
            (),
        )
        .await
        .context("init finalizations archive")?;

        let finalized_blocks = ArchiveInitializer::init::<_, ConsensusDigest, Block>(
            context.with_label("finalized_blocks"),
            format!("{partition_prefix}-finalized-blocks"),
            block_cfg,
        )
        .await
        .context("init blocks archive")?;

        let (actor, marshal_mailbox, _last_processed_height) =
            kora_marshal::ActorInitializer::init::<_, Block, _, _, _, Exact>(
                context.clone(),
                finalizations_by_height,
                finalized_blocks,
                scheme_provider,
                buffer_pool.clone(),
                block_cfg,
            )
            .await;
        actor.start(finalized_reporter, buffer, resolver);

        let epocher = FixedEpocher::new(NZU64!(EPOCH_LENGTH));
        let executor = RevmExecutor::new(self.chain_id);
        let mut app = RevmApplication::<ThresholdScheme, _>::new(
            ledger.clone(),
            executor,
            block_cfg.max_txs,
            self.gas_limit,
        );
        if let Some((state, _)) = &self.rpc_config {
            app = app.with_node_state(state.clone());
        }
        let marshaled =
            Marshaled::new(context.with_label("marshaled"), app, marshal_mailbox.clone(), epocher);

        let seed_reporter = SeedReporter::<MinSig>::new(ledger.clone());
        let node_state_reporter = self
            .rpc_config
            .as_ref()
            .map(|(state, _)| NodeStateReporter::<ThresholdScheme>::new(state.clone()));
        let inner_reporters: Reporters<_, MarshalMailbox, Option<NodeStateRptr>> =
            Reporters::from((marshal_mailbox.clone(), node_state_reporter));
        let reporter = Reporters::from((seed_reporter, inner_reporters));

        for tx in &self.bootstrap.bootstrap_txs {
            let _ = ledger.submit_tx(tx.clone()).await;
        }

        let engine = simplex::Engine::new(
            context.with_label("engine"),
            simplex::Config {
                scheme: self.scheme.clone(),
                elector: Random,
                blocker: transport.oracle.clone(),
                automaton: marshaled.clone(),
                relay: marshaled,
                reporter,
                strategy: Sequential,
                partition: self.partition_prefix.clone(),
                mailbox_size: MAILBOX_SIZE,
                epoch: Epoch::zero(),
                replay_buffer: NZUsize!(1024 * 1024),
                write_buffer: NZUsize!(1024 * 1024),
                leader_timeout: Duration::from_millis(500),
                notarization_timeout: Duration::from_secs(1),
                nullify_retry: Duration::from_secs(2),
                fetch_timeout: Duration::from_millis(500),
                activity_timeout: ViewDelta::new(20),
                skip_timeout: ViewDelta::new(10),
                fetch_concurrent: 8,
                buffer_pool,
            },
        );
        engine.start(transport.simplex.votes, transport.simplex.certs, transport.simplex.resolver);

        info!("Validator started successfully");
        Ok(ledger)
    }
}
