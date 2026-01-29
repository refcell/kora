use std::{fmt, sync::Arc, time::Duration};

use alloy_consensus::Header;
use alloy_primitives::Address;
use anyhow::Context as _;
use commonware_consensus::{
    Reporter, Reporters,
    application::marshaled::Marshaled,
    marshal,
    simplex::{self, elector::Random, types::Finalization},
    types::{Epoch, FixedEpocher, ViewDelta},
};
use commonware_cryptography::bls12381::primitives::variant::MinSig;
use commonware_p2p::simulated;
use commonware_parallel::Sequential;
use commonware_runtime::{Metrics as _, Quota, Spawner, buffer::PoolRef, tokio};
use commonware_utils::{NZU64, NZUsize, acknowledgement::Exact};
use futures::{StreamExt as _, channel::mpsc};
use kora_crypto::ThresholdScheme;
use kora_domain::{
    Block, BlockCfg, BootstrapConfig, ConsensusDigest, FinalizationEvent, LedgerEvent, PublicKey,
    TxCfg,
};
use kora_executor::{BlockContext, RevmExecutor};
use kora_ledger::{LedgerService, LedgerView};
use kora_marshal::{ArchiveInitializer, BroadcastInitializer, PeerInitializer};
use kora_reporters::{BlockContextProvider, FinalizedReporter, SeedReporter};
use kora_service::{NodeRunContext, NodeRunner};
use kora_simplex::{DEFAULT_MAILBOX_SIZE as MAILBOX_SIZE, DefaultPool, DefaultQuota};
use kora_transport_sim::{SimContext, register_node_channels};
use tracing::{debug, trace};

use crate::{
    app::{BLOCK_GAS_LIMIT, CHAIN_ID, RevmApplication},
    handle::NodeHandle,
};

const BLOCK_CODEC_MAX_TXS: usize = 64;
const BLOCK_CODEC_MAX_TX_BYTES: usize = 1024;
const EPOCH_LENGTH: u64 = u64::MAX;
const PARTITION_PREFIX: &str = "revm";

type Peer = PublicKey;
type ChannelSender = simulated::Sender<Peer, SimContext>;
type ChannelReceiver = simulated::Receiver<Peer>;
type CertArchive = Finalization<ThresholdScheme, ConsensusDigest>;

fn default_quota() -> Quota {
    DefaultQuota::init()
}

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

struct MarshalStart<M, R> {
    index: usize,
    partition_prefix: String,
    public_key: Peer,
    control: simulated::Control<Peer, SimContext>,
    manager: M,
    scheme: ThresholdScheme,
    buffer_pool: PoolRef,
    block_codec_config: BlockCfg,
    blocks: (ChannelSender, ChannelReceiver),
    backfill: (ChannelSender, ChannelReceiver),
    application: R,
}

async fn start_marshal<M, R>(
    context: &tokio::Context,
    start: MarshalStart<M, R>,
) -> anyhow::Result<marshal::Mailbox<ThresholdScheme, Block>>
where
    M: commonware_p2p::Manager<PublicKey = Peer>,
    R: Reporter<Activity = marshal::Update<Block>> + Send + 'static,
{
    let MarshalStart {
        index,
        partition_prefix,
        public_key,
        control,
        manager,
        scheme,
        buffer_pool,
        block_codec_config,
        blocks,
        backfill,
        application,
    } = start;

    let ctx = context.with_label(&format!("marshal_{index}"));
    let partition_prefix = format!("{partition_prefix}-marshal-{index}");
    let scheme_provider = ConstantSchemeProvider::from(scheme);

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

    let cert_codec_config =
        <ThresholdScheme as commonware_cryptography::certificate::Scheme>::certificate_codec_config_unbounded();
    let finalizations_by_height = ArchiveInitializer::init::<_, ConsensusDigest, CertArchive>(
        ctx.with_label("finalizations_by_height"),
        format!("{partition_prefix}-finalizations-by-height"),
        cert_codec_config,
    )
    .await
    .context("init finalizations archive")?;

    let finalized_blocks = ArchiveInitializer::init::<_, ConsensusDigest, Block>(
        ctx.with_label("finalized_blocks"),
        format!("{partition_prefix}-finalized-blocks"),
        block_codec_config,
    )
    .await
    .context("init blocks archive")?;

    let (actor, mailbox, _last_processed_height) =
        kora_marshal::ActorInitializer::init::<_, Block, _, _, _, Exact>(
            ctx.clone(),
            finalizations_by_height,
            finalized_blocks,
            scheme_provider,
            buffer_pool,
            block_codec_config,
        )
        .await;
    actor.start(application, buffer, resolver);

    Ok(mailbox)
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

#[derive(Debug)]
pub(crate) struct RunnerError(anyhow::Error);

impl fmt::Display for RunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for RunnerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl From<anyhow::Error> for RunnerError {
    fn from(e: anyhow::Error) -> Self {
        Self(e)
    }
}

#[derive(Clone, Debug)]
struct RevmContextProvider {
    gas_limit: u64,
}

impl RevmContextProvider {
    const fn new(gas_limit: u64) -> Self {
        Self { gas_limit }
    }
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
        BlockContext::new(header, block.prevrandao)
    }
}

#[derive(Clone)]
pub(crate) struct RevmNodeRunner {
    pub(crate) index: usize,
    pub(crate) public_key: PublicKey,
    pub(crate) scheme: ThresholdScheme,
    pub(crate) bootstrap: BootstrapConfig,
    pub(crate) finalized_tx: mpsc::UnboundedSender<FinalizationEvent>,
    pub(crate) manager: simulated::Manager<PublicKey, SimContext>,
}

impl NodeRunner for RevmNodeRunner {
    type Transport = simulated::Control<PublicKey, SimContext>;
    type Handle = NodeHandle;
    type Error = RunnerError;

    async fn run(&self, ctx: NodeRunContext<Self::Transport>) -> Result<Self::Handle, Self::Error> {
        let (context, _config, mut control) = ctx.into_parts();

        let quota = default_quota();
        let buffer_pool = default_buffer_pool();
        let partition_prefix = PARTITION_PREFIX;
        let index = self.index;

        let blocker = control.clone();

        let channels = register_node_channels(&mut control, quota)
            .await
            .map_err(|e| anyhow::anyhow!("channel registration failed: {e}"))?;

        let block_cfg = block_codec_cfg();
        let state = LedgerView::init(
            context.with_label(&format!("state_{index}")),
            buffer_pool.clone(),
            format!("{partition_prefix}-qmdb-{index}"),
            self.bootstrap.genesis_alloc.clone(),
        )
        .await
        .context("init qmdb")?;

        let ledger = LedgerService::new(state.clone());
        spawn_ledger_observers(ledger.clone(), context.clone());
        let mut domain_events = ledger.subscribe();
        let finalized_tx_clone = self.finalized_tx.clone();
        let node_id = index as u32;
        let event_context = context.clone();
        event_context.spawn(move |_| async move {
            while let Some(event) = domain_events.next().await {
                if let LedgerEvent::SnapshotPersisted(digest) = event {
                    let _ = finalized_tx_clone.unbounded_send((node_id, digest));
                }
            }
        });
        let handle = NodeHandle::new(ledger.clone());
        let app = RevmApplication::<ThresholdScheme>::new(block_cfg.max_txs, state.clone());

        let executor = RevmExecutor::new(CHAIN_ID);
        let context_provider = RevmContextProvider::new(BLOCK_GAS_LIMIT);
        let finalized_reporter =
            FinalizedReporter::new(ledger.clone(), context.clone(), executor, context_provider);

        let marshal_mailbox = start_marshal(
            &context,
            MarshalStart {
                index,
                partition_prefix: partition_prefix.to_string(),
                public_key: self.public_key.clone(),
                control: control.clone(),
                manager: self.manager.clone(),
                scheme: self.scheme.clone(),
                buffer_pool: buffer_pool.clone(),
                block_codec_config: block_cfg,
                blocks: channels.marshal.blocks,
                backfill: channels.marshal.backfill,
                application: finalized_reporter,
            },
        )
        .await?;

        let epocher = FixedEpocher::new(NZU64!(EPOCH_LENGTH));
        let marshaled = Marshaled::new(
            context.with_label(&format!("marshaled_{index}")),
            app,
            marshal_mailbox.clone(),
            epocher,
        );

        let seed_reporter = SeedReporter::<MinSig>::new(ledger.clone());
        let reporter = Reporters::from((seed_reporter, marshal_mailbox.clone()));

        for tx in &self.bootstrap.bootstrap_txs {
            let _ = handle.submit_tx(tx.clone()).await;
        }

        let engine = simplex::Engine::new(
            context.with_label(&format!("engine_{index}")),
            simplex::Config {
                scheme: self.scheme.clone(),
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

        Ok(handle)
    }
}
