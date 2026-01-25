//! Execution outcome types.

use alloy_primitives::{Address, B256, Bytes};
use kora_qmdb::ChangeSet;

/// Result of executing a block's transactions.
#[derive(Clone, Debug, Default)]
pub struct ExecutionOutcome {
    /// State changes from execution.
    pub changes: ChangeSet,
    /// Transaction receipts.
    pub receipts: Vec<TransactionReceipt>,
    /// Total gas used by all transactions.
    pub gas_used: u64,
}

impl ExecutionOutcome {
    /// Create a new empty execution outcome.
    pub fn new() -> Self {
        Self { changes: ChangeSet::new(), receipts: Vec::new(), gas_used: 0 }
    }
}

/// Receipt for a single transaction execution.
#[derive(Clone, Debug)]
pub struct TransactionReceipt {
    /// Transaction hash.
    pub tx_hash: B256,
    /// Whether execution succeeded.
    pub success: bool,
    /// Gas used by this transaction.
    pub gas_used: u64,
    /// Cumulative gas used up to and including this transaction.
    pub cumulative_gas_used: u64,
    /// Logs emitted during execution.
    pub logs: Vec<Log>,
    /// Contract address if this was a contract creation.
    pub contract_address: Option<Address>,
}

/// Event log emitted during execution.
#[derive(Clone, Debug)]
pub struct Log {
    /// Address that emitted the log.
    pub address: Address,
    /// Indexed topics.
    pub topics: Vec<B256>,
    /// Log data.
    pub data: Bytes,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_outcome_default() {
        let outcome = ExecutionOutcome::new();
        assert!(outcome.changes.is_empty());
        assert!(outcome.receipts.is_empty());
        assert_eq!(outcome.gas_used, 0);
    }
}
