//! Error types for consensus operations.

use kora_domain::{ConsensusDigest, StateRoot};
use thiserror::Error;

/// Error type for consensus operations.
#[derive(Debug, Error)]
pub enum ConsensusError {
    /// Parent block not found.
    #[error("parent not found: {0:?}")]
    ParentNotFound(ConsensusDigest),

    /// Snapshot not found for digest.
    #[error("snapshot not found: {0:?}")]
    SnapshotNotFound(ConsensusDigest),

    /// Execution failed.
    #[error("execution failed: {0}")]
    Execution(String),

    /// State database error.
    #[error("state db error: {0}")]
    StateDb(#[from] kora_traits::StateDbError),

    /// Block validation failed.
    #[error("validation failed: {0}")]
    Validation(String),

    /// State root mismatch.
    #[error("state root mismatch: expected {expected:?}, got {actual:?}")]
    StateRootMismatch {
        /// Expected state root.
        expected: StateRoot,
        /// Actual state root.
        actual: StateRoot,
    },
}

#[cfg(test)]
mod tests {
    use alloy_primitives::B256;

    use super::*;

    fn test_digest() -> ConsensusDigest {
        ConsensusDigest::from([0u8; 32])
    }

    #[test]
    fn test_parent_not_found_display() {
        let digest = test_digest();
        let err = ConsensusError::ParentNotFound(digest);
        let msg = err.to_string();
        assert!(msg.starts_with("parent not found:"));
    }

    #[test]
    fn test_snapshot_not_found_display() {
        let digest = test_digest();
        let err = ConsensusError::SnapshotNotFound(digest);
        let msg = err.to_string();
        assert!(msg.starts_with("snapshot not found:"));
    }

    #[test]
    fn test_execution_display() {
        let err = ConsensusError::Execution("out of gas".to_string());
        assert_eq!(err.to_string(), "execution failed: out of gas");
    }

    #[test]
    fn test_state_db_error_from() {
        let state_err = kora_traits::StateDbError::LockPoisoned;
        let err: ConsensusError = state_err.into();
        assert!(err.to_string().contains("state db error"));
    }

    #[test]
    fn test_validation_display() {
        let err = ConsensusError::Validation("invalid block hash".to_string());
        assert_eq!(err.to_string(), "validation failed: invalid block hash");
    }

    #[test]
    fn test_state_root_mismatch_display() {
        let expected = StateRoot(B256::ZERO);
        let actual = StateRoot(B256::repeat_byte(0xff));
        let err = ConsensusError::StateRootMismatch { expected, actual };
        let msg = err.to_string();
        assert!(msg.contains("state root mismatch"));
        assert!(msg.contains("expected"));
        assert!(msg.contains("got"));
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ConsensusError>();
    }

    #[test]
    fn test_error_debug_impl() {
        let err = ConsensusError::Execution("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Execution"));
    }
}
