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

#[cfg(test)]
mod tests {
    use alloy_primitives::B256;

    use super::*;

    #[test]
    fn test_state_error_display() {
        let state_err = kora_traits::StateDbError::LockPoisoned;
        let err = ExecutionError::State(state_err);
        assert_eq!(err.to_string(), "state error: lock poisoned");
    }

    #[test]
    fn test_state_error_from() {
        let state_err = kora_traits::StateDbError::LockPoisoned;
        let err: ExecutionError = state_err.into();
        assert!(matches!(err, ExecutionError::State(_)));
    }

    #[test]
    fn test_tx_decode_display() {
        let err = ExecutionError::TxDecode("invalid RLP".to_string());
        assert_eq!(err.to_string(), "failed to decode transaction: invalid RLP");
    }

    #[test]
    fn test_tx_execution_display() {
        let err = ExecutionError::TxExecution("out of gas".to_string());
        assert_eq!(err.to_string(), "transaction execution failed: out of gas");
    }

    #[test]
    fn test_invalid_tx_display() {
        let err = ExecutionError::InvalidTx("nonce too low".to_string());
        assert_eq!(err.to_string(), "invalid transaction: nonce too low");
    }

    #[test]
    fn test_block_validation_display() {
        let err = ExecutionError::BlockValidation("invalid state root".to_string());
        assert_eq!(err.to_string(), "block validation failed: invalid state root");
    }

    #[test]
    fn test_code_not_found_display() {
        let hash = B256::ZERO;
        let err = ExecutionError::CodeNotFound(hash);
        assert_eq!(err.to_string(), format!("code not found: {hash}"));
    }

    #[test]
    fn test_error_debug_impl() {
        let err = ExecutionError::TxDecode("test".to_string());
        assert!(format!("{:?}", err).contains("TxDecode"));
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ExecutionError>();
    }
}
