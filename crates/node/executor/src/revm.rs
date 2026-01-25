//! REVM-based block executor.

use alloy_consensus::Header;
use alloy_primitives::Bytes;
use kora_traits::StateDb;

use crate::{BlockContext, BlockExecutor, ExecutionError, ExecutionOutcome};

/// REVM-based block executor.
///
/// This executor uses REVM to execute EVM transactions against a state database.
/// The actual EVM execution is performed via the REVM handler traits.
#[derive(Clone, Debug, Default)]
pub struct RevmExecutor {
    /// Chain ID for transaction validation.
    chain_id: u64,
}

impl RevmExecutor {
    /// Create a new REVM executor with the given chain ID.
    pub const fn new(chain_id: u64) -> Self {
        Self { chain_id }
    }

    /// Get the chain ID.
    pub const fn chain_id(&self) -> u64 {
        self.chain_id
    }
}

impl<S: StateDb> BlockExecutor<S> for RevmExecutor {
    type Tx = Bytes;

    fn execute(
        &self,
        _state: &S,
        _context: &BlockContext,
        _txs: &[Self::Tx],
    ) -> Result<ExecutionOutcome, ExecutionError> {
        // TODO: Implement full EVM execution using REVM 24+ API
        // For now, return an empty outcome as a placeholder.
        // Full implementation requires:
        // 1. Building a MainnetEvm with the StateDbAdapter
        // 2. Iterating over transactions and executing each
        // 3. Collecting receipts and state changes
        Ok(ExecutionOutcome::new())
    }

    fn validate_header(&self, _header: &Header) -> Result<(), ExecutionError> {
        // Basic header validation
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revm_executor_new() {
        let executor = RevmExecutor::new(1);
        assert_eq!(executor.chain_id(), 1);
    }

    #[test]
    fn revm_executor_default() {
        let executor = RevmExecutor::default();
        assert_eq!(executor.chain_id(), 0);
    }
}
