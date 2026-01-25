//! Execution error types.

use alloy_primitives::B256;
use revm::database_interface::DBErrorMarker;
use thiserror::Error;

/// Errors that can occur during block execution.
#[derive(Debug, Error)]
pub enum ExecutionError {
    /// State database error.
    #[error("state error: {0}")]
    State(#[from] kora_traits::StateDbError),

    /// Transaction decoding failed.
    #[error("failed to decode transaction: {0}")]
    TxDecode(String),

    /// Transaction execution failed.
    #[error("transaction execution failed: {0}")]
    TxExecution(String),

    /// Invalid transaction.
    #[error("invalid transaction: {0}")]
    InvalidTx(String),

    /// Block validation failed.
    #[error("block validation failed: {0}")]
    BlockValidation(String),

    /// Code not found for hash.
    #[error("code not found: {0}")]
    CodeNotFound(B256),
}

impl DBErrorMarker for ExecutionError {}
