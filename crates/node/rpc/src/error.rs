//! JSON-RPC error types following Ethereum error code conventions.

use jsonrpsee::types::ErrorObjectOwned;
use thiserror::Error;

/// JSON-RPC error codes following Ethereum conventions.
pub mod codes {
    /// Invalid JSON was received.
    pub const PARSE_ERROR: i32 = -32700;
    /// The JSON sent is not a valid Request object.
    pub const INVALID_REQUEST: i32 = -32600;
    /// The method does not exist / is not available.
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// Invalid method parameter(s).
    pub const INVALID_PARAMS: i32 = -32602;
    /// Internal JSON-RPC error.
    pub const INTERNAL_ERROR: i32 = -32603;

    /// Server error (reserved range: -32000 to -32099).
    pub const SERVER_ERROR: i32 = -32000;
    /// Resource not found.
    pub const RESOURCE_NOT_FOUND: i32 = -32001;
    /// Resource unavailable.
    pub const RESOURCE_UNAVAILABLE: i32 = -32002;
    /// Transaction rejected.
    pub const TRANSACTION_REJECTED: i32 = -32003;
    /// Method not supported.
    pub const METHOD_NOT_SUPPORTED: i32 = -32004;
    /// Request limit exceeded.
    pub const LIMIT_EXCEEDED: i32 = -32005;
    /// Execution error (revert, out of gas, etc.).
    pub const EXECUTION_ERROR: i32 = -32015;
}

/// RPC-specific errors that can occur during request handling.
#[derive(Debug, Error)]
pub enum RpcError {
    /// Block not found.
    #[error("block not found")]
    BlockNotFound,

    /// Transaction not found.
    #[error("transaction not found")]
    TransactionNotFound,

    /// Account not found.
    #[error("account not found: {0}")]
    AccountNotFound(String),

    /// Invalid block number.
    #[error("invalid block number: {0}")]
    InvalidBlockNumber(String),

    /// Invalid transaction.
    #[error("invalid transaction: {0}")]
    InvalidTransaction(String),

    /// Execution failed.
    #[error("execution failed: {0}")]
    ExecutionFailed(String),

    /// State database error.
    #[error("state error: {0}")]
    StateError(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),

    /// Method not implemented.
    #[error("method not implemented")]
    NotImplemented,
}

impl From<RpcError> for ErrorObjectOwned {
    fn from(err: RpcError) -> Self {
        let (code, message) = match &err {
            RpcError::BlockNotFound => (codes::RESOURCE_NOT_FOUND, err.to_string()),
            RpcError::TransactionNotFound => (codes::RESOURCE_NOT_FOUND, err.to_string()),
            RpcError::AccountNotFound(_) => (codes::RESOURCE_NOT_FOUND, err.to_string()),
            RpcError::InvalidBlockNumber(_) => (codes::INVALID_PARAMS, err.to_string()),
            RpcError::InvalidTransaction(_) => (codes::INVALID_PARAMS, err.to_string()),
            RpcError::ExecutionFailed(_) => (codes::EXECUTION_ERROR, err.to_string()),
            RpcError::StateError(_) => (codes::INTERNAL_ERROR, err.to_string()),
            RpcError::Internal(_) => (codes::INTERNAL_ERROR, err.to_string()),
            RpcError::NotImplemented => (codes::METHOD_NOT_SUPPORTED, err.to_string()),
        };
        ErrorObjectOwned::owned(code, message, None::<()>)
    }
}
