//! Core execution traits.

use alloy_consensus::Header;
use kora_traits::StateDb;

use crate::{BlockContext, ExecutionError, ExecutionOutcome};

/// Executes transactions against a state database.
///
/// Abstracts the EVM execution layer to allow different backends.
pub trait BlockExecutor<S: StateDb>: Clone + Send + Sync + 'static {
    /// Transaction type accepted for execution.
    type Tx: Clone + Send + Sync + 'static;

    /// Execute a batch of transactions against the given state.
    ///
    /// Returns the execution outcome containing state changes and receipts.
    fn execute(
        &self,
        state: &S,
        context: &BlockContext,
        txs: &[Self::Tx],
    ) -> Result<ExecutionOutcome, ExecutionError>;

    /// Validate a block header.
    fn validate_header(&self, header: &Header) -> Result<(), ExecutionError>;
}
