//! Error types for QMDB operations.

use alloy_primitives::B256;
use thiserror::Error;

/// Error type for QMDB store operations.
#[derive(Debug, Error)]
pub enum QmdbError {
    /// Storage backend error.
    #[error("storage error: {0}")]
    Storage(String),

    /// Stores unavailable during update.
    #[error("stores unavailable")]
    StoreUnavailable,

    /// Account decoding failed.
    #[error("account decode failed")]
    DecodeError,

    /// Code not found for hash.
    #[error("code not found: {0}")]
    CodeNotFound(B256),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_error_display() {
        let err = QmdbError::Storage("disk full".to_string());
        assert_eq!(err.to_string(), "storage error: disk full");
    }

    #[test]
    fn test_store_unavailable_display() {
        let err = QmdbError::StoreUnavailable;
        assert_eq!(err.to_string(), "stores unavailable");
    }

    #[test]
    fn test_decode_error_display() {
        let err = QmdbError::DecodeError;
        assert_eq!(err.to_string(), "account decode failed");
    }

    #[test]
    fn test_code_not_found_display() {
        let hash = B256::ZERO;
        let err = QmdbError::CodeNotFound(hash);
        assert!(err.to_string().contains("code not found"));
        assert!(err.to_string().contains(&hash.to_string()));
    }

    #[test]
    fn test_qmdb_error_debug() {
        let err = QmdbError::StoreUnavailable;
        let debug = format!("{:?}", err);
        assert!(debug.contains("StoreUnavailable"));
    }

    #[test]
    fn test_qmdb_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<QmdbError>();
    }
}
