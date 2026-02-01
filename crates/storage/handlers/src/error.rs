//! Error types for database handles.

use alloy_primitives::B256;
use kora_qmdb::QmdbError;
use thiserror::Error;

/// Error type for database handle operations.
#[derive(Debug, Error)]
pub enum HandleError {
    /// QMDB store error.
    #[error("qmdb error: {0}")]
    Qmdb(#[from] QmdbError),

    /// Lock was poisoned.
    #[error("lock poisoned")]
    LockPoisoned,

    /// Code not found for hash.
    #[error("code not found: {0}")]
    CodeNotFound(B256),

    /// Block hash not found.
    #[error("block hash not found: {0}")]
    BlockHashNotFound(u64),

    /// Root computation error.
    #[error("root computation error: {0}")]
    RootComputation(String),
}

impl revm::database_interface::DBErrorMarker for HandleError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hash() -> B256 {
        B256::repeat_byte(0xab)
    }

    #[test]
    fn test_qmdb_error_display() {
        let inner = QmdbError::StoreUnavailable;
        let err = HandleError::Qmdb(inner);
        assert!(err.to_string().contains("qmdb error"));
    }

    #[test]
    fn test_qmdb_error_from() {
        let inner = QmdbError::StoreUnavailable;
        let err: HandleError = inner.into();
        assert!(matches!(err, HandleError::Qmdb(_)));
    }

    #[test]
    fn test_lock_poisoned_display() {
        let err = HandleError::LockPoisoned;
        assert_eq!(err.to_string(), "lock poisoned");
    }

    #[test]
    fn test_code_not_found_display() {
        let hash = test_hash();
        let err = HandleError::CodeNotFound(hash);
        let msg = err.to_string();
        assert!(msg.starts_with("code not found:"));
        assert!(msg.contains(&format!("{hash}")));
    }

    #[test]
    fn test_block_hash_not_found_display() {
        let err = HandleError::BlockHashNotFound(12_345);
        assert_eq!(err.to_string(), "block hash not found: 12345");
    }

    #[test]
    fn test_root_computation_display() {
        let err = HandleError::RootComputation("test error".to_string());
        assert_eq!(err.to_string(), "root computation error: test error");
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<HandleError>();
    }

    #[test]
    fn test_error_debug_impl() {
        let err = HandleError::LockPoisoned;
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("LockPoisoned"));
    }
}
