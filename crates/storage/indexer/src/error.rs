//! Error types for the indexer.

use alloy_primitives::B256;
use thiserror::Error;

/// Errors that can occur during indexing operations.
#[derive(Debug, Error)]
pub enum IndexerError {
    /// Block not found by hash.
    #[error("block not found: {0}")]
    BlockNotFound(B256),

    /// Block not found by number.
    #[error("block not found at height: {0}")]
    BlockNotFoundByNumber(u64),

    /// Transaction not found.
    #[error("transaction not found: {0}")]
    TransactionNotFound(B256),

    /// Receipt not found.
    #[error("receipt not found: {0}")]
    ReceiptNotFound(B256),

    /// Invalid block range for log filter.
    #[error("invalid block range: from {from} > to {to}")]
    InvalidBlockRange {
        /// Start of the range.
        from: u64,
        /// End of the range.
        to: u64,
    },
}
