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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hash() -> B256 {
        B256::repeat_byte(0xab)
    }

    #[test]
    fn test_block_not_found_display() {
        let hash = test_hash();
        let err = IndexerError::BlockNotFound(hash);
        let msg = err.to_string();
        assert!(msg.starts_with("block not found:"));
        assert!(msg.contains(&format!("{hash}")));
    }

    #[test]
    fn test_block_not_found_by_number_display() {
        let err = IndexerError::BlockNotFoundByNumber(12_345);
        assert_eq!(err.to_string(), "block not found at height: 12345");
    }

    #[test]
    fn test_transaction_not_found_display() {
        let hash = test_hash();
        let err = IndexerError::TransactionNotFound(hash);
        let msg = err.to_string();
        assert!(msg.starts_with("transaction not found:"));
        assert!(msg.contains(&format!("{hash}")));
    }

    #[test]
    fn test_receipt_not_found_display() {
        let hash = test_hash();
        let err = IndexerError::ReceiptNotFound(hash);
        let msg = err.to_string();
        assert!(msg.starts_with("receipt not found:"));
        assert!(msg.contains(&format!("{hash}")));
    }

    #[test]
    fn test_invalid_block_range_display() {
        let err = IndexerError::InvalidBlockRange { from: 100, to: 50 };
        assert_eq!(err.to_string(), "invalid block range: from 100 > to 50");
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<IndexerError>();
    }

    #[test]
    fn test_error_debug_impl() {
        let err = IndexerError::BlockNotFoundByNumber(1);
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("BlockNotFoundByNumber"));
    }
}
