//! Error types for the backend.

use thiserror::Error;

/// Error type for backend operations.
#[derive(Debug, Error)]
pub enum BackendError {
    /// Storage I/O error.
    #[error("storage error: {0}")]
    Storage(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// Database not initialized.
    #[error("database not initialized")]
    NotInitialized,

    /// Partition error.
    #[error("partition error: {0}")]
    Partition(String),

    /// State root computation failed.
    #[error("root computation failed: {0}")]
    RootComputation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_error_display() {
        let err = BackendError::Storage("disk full".to_string());
        assert_eq!(err.to_string(), "storage error: disk full");
    }

    #[test]
    fn test_config_error_display() {
        let err = BackendError::Config("invalid path".to_string());
        assert_eq!(err.to_string(), "configuration error: invalid path");
    }

    #[test]
    fn test_not_initialized_display() {
        let err = BackendError::NotInitialized;
        assert_eq!(err.to_string(), "database not initialized");
    }

    #[test]
    fn test_partition_error_display() {
        let err = BackendError::Partition("corrupted".to_string());
        assert_eq!(err.to_string(), "partition error: corrupted");
    }

    #[test]
    fn test_root_computation_error_display() {
        let err = BackendError::RootComputation("merkle tree failed".to_string());
        assert_eq!(err.to_string(), "root computation failed: merkle tree failed");
    }

    #[test]
    fn test_backend_error_debug() {
        let err = BackendError::NotInitialized;
        let debug = format!("{:?}", err);
        assert!(debug.contains("NotInitialized"));
    }

    #[test]
    fn test_backend_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BackendError>();
    }
}
