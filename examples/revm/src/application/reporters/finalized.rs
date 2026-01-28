use alloy_consensus::Header;
use alloy_primitives::{Address, B256};
use commonware_consensus::{Block as _, Reporter, marshal::Update};
use commonware_cryptography::Committable as _;
use commonware_runtime::{Spawner as _, tokio};
use commonware_utils::acknowledgement::Acknowledgement as _;
use kora_consensus::BlockExecution;
use kora_domain::Block;
use kora_executor::{BlockContext, RevmExecutor};
use kora_overlay::OverlayState;
use tracing::{error, trace, warn};

use super::super::ledger::LedgerService;
use crate::chain::CHAIN_ID;
const BLOCK_GAS_LIMIT: u64 = 30_000_000;

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

/// Helper function for `FinalizedReporter::report` that owns all its inputs.
async fn handle_finalized_update(
    state: LedgerService,
    context: tokio::Context,
    update: Update<Block>,
) {
    match update {
        Update::Tip(..) => {}
        Update::Block(block, ack) => {
            let digest = block.commitment();
            if state.query_state_root(digest).await.is_none() {
                trace!(?digest, "missing snapshot for finalized block; re-executing");
                let parent_digest = block.parent();
                let Some(parent_snapshot) = state.parent_snapshot(parent_digest).await else {
                    error!(?digest, ?parent_digest, "missing parent snapshot for finalized block");
                    ack.acknowledge();
                    return;
                };
                let executor = RevmExecutor::new(CHAIN_ID);
                let context = block_context(block.height, block.prevrandao);
                let execution = match BlockExecution::execute(
                    &parent_snapshot,
                    &executor,
                    &context,
                    &block.txs,
                )
                .await
                {
                    Ok(result) => result,
                    Err(err) => {
                        error!(?digest, error = ?err, "failed to execute finalized block");
                        ack.acknowledge();
                        return;
                    }
                };
                let merged_changes =
                    parent_snapshot.state.merge_changes(execution.outcome.changes.clone());
                let state_root = match state
                    .compute_root_from_store(parent_digest, execution.outcome.changes.clone())
                    .await
                {
                    Ok(root) => root,
                    Err(err) => {
                        error!(?digest, error = ?err, "failed to compute qmdb root");
                        ack.acknowledge();
                        return;
                    }
                };
                if state_root != block.state_root {
                    warn!(
                        ?digest,
                        expected = ?block.state_root,
                        computed = ?state_root,
                        "state root mismatch for finalized block"
                    );
                    ack.acknowledge();
                    return;
                }
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
            } else {
                trace!(?digest, "using cached snapshot for finalized block");
            }
            let persist_state = state.clone();
            let persist_handle = context
                .shared(true)
                .spawn(move |_| async move { persist_state.persist_snapshot(digest).await });
            let persist_result = match persist_handle.await {
                Ok(result) => result,
                Err(err) => Err(err.into()),
            };
            if let Err(err) = persist_result {
                error!(?digest, error = ?err, "failed to persist finalized block");
                ack.acknowledge();
                return;
            }
            state.prune_mempool(&block.txs).await;
            // Marshal waits for the application to acknowledge processing before advancing the
            // delivery floor. Without this, the node can stall on finalized block delivery.
            ack.acknowledge();
        }
    }
}

#[derive(Clone)]
/// Persists finalized blocks.
pub(crate) struct FinalizedReporter {
    /// Ledger service used to verify blocks and persist snapshots.
    state: LedgerService,
    /// Tokio context used to schedule blocking work.
    context: tokio::Context,
}

impl FinalizedReporter {
    pub(crate) const fn new(state: LedgerService, context: tokio::Context) -> Self {
        Self { state, context }
    }
}

impl Reporter for FinalizedReporter {
    type Activity = Update<Block>;

    fn report(&mut self, update: Self::Activity) -> impl std::future::Future<Output = ()> + Send {
        let state = self.state.clone();
        let context = self.context.clone();
        async move {
            handle_finalized_update(state, context, update).await;
        }
    }
}
