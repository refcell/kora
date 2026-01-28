//! Shared block execution helpers.

use alloy_primitives::Bytes;
use kora_domain::{StateRoot, Tx};
use kora_executor::{BlockContext, BlockExecutor, ExecutionOutcome};
use kora_traits::StateDb;

use crate::{ConsensusError, Snapshot};

/// Result of executing a block against a parent snapshot.
#[derive(Debug)]
pub struct BlockExecution {
    /// Execution outcome, including changes and receipts.
    pub outcome: ExecutionOutcome,
    /// Computed state root after applying the execution changes.
    pub state_root: StateRoot,
}

impl BlockExecution {
    /// Execute a block's transactions against a parent snapshot.
    ///
    /// This helper runs the executor, computes the new state root, and returns the
    /// execution outcome for callers to persist or cache.
    pub async fn execute<S, E>(
        parent_snapshot: &Snapshot<S>,
        executor: &E,
        context: &BlockContext,
        txs: &[Tx],
    ) -> Result<Self, ConsensusError>
    where
        S: StateDb,
        E: BlockExecutor<S, Tx = Bytes>,
    {
        let txs_bytes: Vec<Bytes> = txs.iter().map(|tx| tx.bytes.clone()).collect();
        let outcome = executor
            .execute(&parent_snapshot.state, context, &txs_bytes)
            .map_err(|e| ConsensusError::Execution(e.to_string()))?;
        let state_root = parent_snapshot
            .state
            .compute_root(&outcome.changes)
            .await
            .map_err(ConsensusError::StateDb)?;
        Ok(Self { outcome, state_root: StateRoot(state_root) })
    }
}
