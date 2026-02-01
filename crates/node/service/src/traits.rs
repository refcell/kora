//! Trait definitions for the node service.

use std::{fmt::Debug, future::Future, pin::Pin};

/// A boxed future for dyn-compatible async trait methods.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Error type for node service operations.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// Node initialization failed.
    #[error("node initialization failed: {0}")]
    InitializationFailed(String),

    /// Node is not running.
    #[error("node not running")]
    NotRunning,

    /// Transaction submission failed.
    #[error("transaction submission failed: {0}")]
    SubmissionFailed(String),

    /// Query failed.
    #[error("query failed: {0}")]
    QueryFailed(String),
}

/// Handle to interact with a running node externally.
///
/// This is the public API for driving the node from outside.
/// Enables transaction submission, state queries, and subscription to events.
///
/// The trait uses boxed futures to be dyn-compatible, allowing
/// different handle implementations to be used interchangeably.
pub trait NodeHandle: Debug + Send + Sync {
    /// Submit a transaction to the node's mempool.
    ///
    /// This is non-blocking and returns immediately. The transaction
    /// will be included in a future proposed block if valid.
    fn submit_tx(&self, tx: Vec<u8>) -> BoxFuture<'_, Result<(), ServiceError>>;

    /// Get the current finalized state root.
    ///
    /// Returns the state root of the last finalized block.
    fn finalized_state_root(&self) -> BoxFuture<'_, Result<Vec<u8>, ServiceError>>;

    /// Get the current finalized block height.
    fn finalized_height(&self) -> BoxFuture<'_, Result<u64, ServiceError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_error_display_initialization_failed() {
        let error = ServiceError::InitializationFailed("test error".to_string());
        assert_eq!(error.to_string(), "node initialization failed: test error");
    }

    #[test]
    fn service_error_display_not_running() {
        let error = ServiceError::NotRunning;
        assert_eq!(error.to_string(), "node not running");
    }

    #[test]
    fn service_error_display_submission_failed() {
        let error = ServiceError::SubmissionFailed("tx error".to_string());
        assert_eq!(error.to_string(), "transaction submission failed: tx error");
    }

    #[test]
    fn service_error_display_query_failed() {
        let error = ServiceError::QueryFailed("query error".to_string());
        assert_eq!(error.to_string(), "query failed: query error");
    }

    #[test]
    fn service_error_debug() {
        let error = ServiceError::NotRunning;
        assert!(format!("{:?}", error).contains("NotRunning"));
    }
}
