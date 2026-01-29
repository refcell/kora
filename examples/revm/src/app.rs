use std::{collections::BTreeSet, marker::PhantomData};

use alloy_consensus::Header;
use alloy_primitives::{Address, B256};
use commonware_consensus::{
    Application, VerifyingApplication,
    marshal::ingress::mailbox::AncestorStream,
    simplex::{scheme::Scheme, types::Context},
};
use commonware_cryptography::{Committable as _, certificate::Scheme as CertScheme};
use commonware_runtime::{Clock, Metrics, Spawner};
use futures::StreamExt as _;
use kora_consensus::{BlockExecution, Mempool, ProposalBuilder};
use kora_domain::{Block, ConsensusDigest, PublicKey, TxId};
use kora_executor::{BlockContext, RevmExecutor};
use kora_ledger::{LedgerService, LedgerView};
use kora_overlay::OverlayState;
use rand::Rng;

pub(crate) const CHAIN_ID: u64 = 1337;
pub(crate) const BLOCK_GAS_LIMIT: u64 = 30_000_000;

fn block_context(height: u64, prevrandao: B256) -> BlockContext {
    let header = Header {
        number: height,
        timestamp: height,
        gas_limit: BLOCK_GAS_LIMIT,
        beneficiary: Address::ZERO,
        base_fee_per_gas: Some(0),
        ..Default::default()
    };
    BlockContext::new(header, prevrandao)
}

#[derive(Clone)]
struct FilteredMempool<M> {
    inner: M,
    excluded: BTreeSet<TxId>,
}

impl<M: Mempool> Mempool for FilteredMempool<M> {
    fn insert(&self, tx: kora_domain::Tx) -> bool {
        self.inner.insert(tx)
    }

    fn build(&self, max_txs: usize, excluded: &BTreeSet<TxId>) -> Vec<kora_domain::Tx> {
        let mut merged = excluded.clone();
        merged.extend(self.excluded.iter().copied());
        self.inner.build(max_txs, &merged)
    }

    fn prune(&self, tx_ids: &[TxId]) {
        self.inner.prune(tx_ids);
    }

    fn len(&self) -> usize {
        self.inner.len()
    }
}

async fn propose_inner<S>(
    state: LedgerService,
    max_txs: usize,
    mut ancestry: AncestorStream<S, Block>,
) -> Option<Block>
where
    S: CertScheme,
{
    let parent = ancestry.next().await?;

    let mut excluded = BTreeSet::<TxId>::new();
    for tx in parent.txs.iter() {
        excluded.insert(tx.id());
    }
    while let Some(block) = ancestry.next().await {
        for tx in block.txs.iter() {
            excluded.insert(tx.id());
        }
    }

    let parent_digest = parent.commitment();
    let seed_hash = state.seed_for_parent(parent_digest).await;
    let prevrandao = seed_hash.unwrap_or_else(|| B256::from(parent_digest.0));

    let (root_state, mempool, snapshots) = state.proposal_components().await;
    let executor = RevmExecutor::new(CHAIN_ID);
    let mempool = FilteredMempool { inner: mempool, excluded };
    let builder =
        ProposalBuilder::new(root_state, mempool, snapshots, executor).with_max_txs(max_txs);
    let (child, proposal_snapshot) =
        builder.build_proposal_async(&parent, prevrandao).await.ok()?;
    let parent_snapshot = state.parent_snapshot(parent_digest).await?;
    let merged_changes = parent_snapshot.state.merge_changes(proposal_snapshot.changes.clone());
    let next_state = OverlayState::new(parent_snapshot.state.base(), merged_changes);
    let digest = child.commitment();
    let snapshot = kora_consensus::Snapshot::new(
        Some(parent_digest),
        next_state,
        proposal_snapshot.state_root,
        proposal_snapshot.changes,
        proposal_snapshot.tx_ids,
    );
    state.cache_snapshot(digest, snapshot).await;
    Some(child)
}

async fn verify_inner<S>(state: LedgerService, mut ancestry: AncestorStream<S, Block>) -> bool
where
    S: CertScheme,
{
    let block = match ancestry.next().await {
        Some(block) => block,
        None => return false,
    };
    let parent = match ancestry.next().await {
        Some(block) => block,
        None => return false,
    };

    let parent_digest = parent.commitment();
    let Some(parent_snapshot) = state.parent_snapshot(parent_digest).await else {
        return false;
    };

    let executor = RevmExecutor::new(CHAIN_ID);
    let context = block_context(block.height, block.prevrandao);
    let execution =
        match BlockExecution::execute(&parent_snapshot, &executor, &context, &block.txs).await {
            Ok(result) => result,
            Err(_) => return false,
        };
    let merged_changes = parent_snapshot.state.merge_changes(execution.outcome.changes.clone());
    let state_root =
        match state.compute_root_from_store(parent_digest, execution.outcome.changes.clone()).await
        {
            Ok(root) => root,
            Err(_) => return false,
        };
    if state_root != block.state_root {
        return false;
    }

    let digest = block.commitment();
    let next_state = OverlayState::new(parent_snapshot.state.base(), merged_changes);
    state
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

#[derive(Clone)]
pub(crate) struct RevmApplication<S> {
    max_txs: usize,
    state: LedgerService,
    _scheme: PhantomData<S>,
}

impl<S> RevmApplication<S> {
    pub(crate) fn new(max_txs: usize, state: LedgerView) -> Self {
        Self { max_txs, state: LedgerService::new(state), _scheme: PhantomData }
    }
}

impl<E, S> Application<E> for RevmApplication<S>
where
    E: Rng + Spawner + Metrics + Clock,
    S: Scheme<ConsensusDigest>
        + commonware_cryptography::certificate::Scheme<PublicKey = PublicKey>,
{
    type SigningScheme = S;
    type Context = Context<ConsensusDigest, PublicKey>;
    type Block = Block;

    fn genesis(&mut self) -> impl std::future::Future<Output = Self::Block> + Send {
        let block = self.state.genesis_block();
        async move { block }
    }

    fn propose(
        &mut self,
        context: (E, Self::Context),
        ancestry: AncestorStream<Self::SigningScheme, Self::Block>,
    ) -> impl std::future::Future<Output = Option<Self::Block>> + Send {
        let state = self.state.clone();
        let max_txs = self.max_txs;
        let _ = context;
        async move { propose_inner(state, max_txs, ancestry).await }
    }
}

impl<E, S> VerifyingApplication<E> for RevmApplication<S>
where
    E: Rng + Spawner + Metrics + Clock,
    S: Scheme<ConsensusDigest>
        + commonware_cryptography::certificate::Scheme<PublicKey = PublicKey>,
{
    fn verify(
        &mut self,
        context: (E, Self::Context),
        ancestry: AncestorStream<Self::SigningScheme, Self::Block>,
    ) -> impl std::future::Future<Output = bool> + Send {
        let state = self.state.clone();
        let _ = context;
        async move { verify_inner(state, ancestry).await }
    }
}
