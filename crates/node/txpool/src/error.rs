//! Transaction pool error types.

use alloy_primitives::{Address, U256};
use thiserror::Error;

/// Errors that can occur during transaction pool operations.
#[derive(Debug, Error)]
pub enum TxPoolError {
    /// The pool has reached its capacity limit.
    #[error("pool is full")]
    PoolFull,

    /// The sender has reached their transaction limit.
    #[error("sender {0} has too many transactions")]
    SenderFull(Address),

    /// The transaction exceeds the maximum allowed size.
    #[error("transaction size {size} exceeds maximum {max}")]
    TxTooLarge {
        /// Actual transaction size.
        size: usize,
        /// Maximum allowed size.
        max: usize,
    },

    /// The gas price is below the minimum required.
    #[error("gas price {price} below minimum {min}")]
    GasPriceTooLow {
        /// Provided gas price.
        price: u128,
        /// Minimum required gas price.
        min: u128,
    },

    /// The transaction nonce is too low.
    #[error("nonce too low: got {got}, expected at least {expected}")]
    NonceTooLow {
        /// Provided nonce.
        got: u64,
        /// Expected minimum nonce.
        expected: u64,
    },

    /// There is a gap in nonces for this sender.
    #[error("nonce gap: got {got}, expected {expected}")]
    NonceGap {
        /// Provided nonce.
        got: u64,
        /// Expected nonce.
        expected: u64,
    },

    /// Insufficient balance to cover transaction costs.
    #[error("insufficient balance: need {need}, have {have}")]
    InsufficientBalance {
        /// Required balance.
        need: U256,
        /// Available balance.
        have: U256,
    },

    /// The chain ID does not match the expected value.
    #[error("invalid chain id: got {got}, expected {expected}")]
    InvalidChainId {
        /// Provided chain ID.
        got: u64,
        /// Expected chain ID.
        expected: u64,
    },

    /// The transaction signature is invalid.
    #[error("invalid signature")]
    InvalidSignature,

    /// Failed to decode the transaction.
    #[error("failed to decode transaction: {0}")]
    DecodeError(String),

    /// The gas limit is below the intrinsic gas cost.
    #[error("gas limit {limit} below intrinsic gas {intrinsic}")]
    IntrinsicGasTooLow {
        /// Provided gas limit.
        limit: u64,
        /// Required intrinsic gas.
        intrinsic: u64,
    },

    /// The transaction already exists in the pool.
    #[error("transaction already exists")]
    AlreadyExists,

    /// An error occurred while accessing state.
    #[error("state error: {0}")]
    StateError(String),

    /// Replacement transaction does not have sufficient gas price bump.
    #[error("replacement transaction underpriced")]
    ReplacementUnderpriced,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_full_display() {
        let err = TxPoolError::PoolFull;
        assert_eq!(err.to_string(), "pool is full");
    }

    #[test]
    fn test_sender_full_display() {
        let addr = Address::repeat_byte(0xab);
        let err = TxPoolError::SenderFull(addr);
        let display = err.to_string();
        assert!(display.contains("has too many transactions"));
        assert!(
            display.contains("abab") || display.contains("AbAb") || display.contains("ABAB"),
            "expected address in display: {}",
            display
        );
    }

    #[test]
    fn test_tx_too_large_display() {
        let err = TxPoolError::TxTooLarge { size: 150000, max: 131072 };
        assert_eq!(err.to_string(), "transaction size 150000 exceeds maximum 131072");
    }

    #[test]
    fn test_gas_price_too_low_display() {
        let err = TxPoolError::GasPriceTooLow { price: 1_000_000_000, min: 2_000_000_000 };
        assert_eq!(err.to_string(), "gas price 1000000000 below minimum 2000000000");
    }

    #[test]
    fn test_nonce_too_low_display() {
        let err = TxPoolError::NonceTooLow { got: 5, expected: 10 };
        assert_eq!(err.to_string(), "nonce too low: got 5, expected at least 10");
    }

    #[test]
    fn test_nonce_gap_display() {
        let err = TxPoolError::NonceGap { got: 15, expected: 10 };
        assert_eq!(err.to_string(), "nonce gap: got 15, expected 10");
    }

    #[test]
    fn test_insufficient_balance_display() {
        let err = TxPoolError::InsufficientBalance {
            need: U256::from(1_000_000_000_000_000_000u128),
            have: U256::from(500_000_000_000_000_000u128),
        };
        let display = err.to_string();
        assert!(display.contains("insufficient balance"));
        assert!(display.contains("need"));
        assert!(display.contains("have"));
    }

    #[test]
    fn test_invalid_chain_id_display() {
        let err = TxPoolError::InvalidChainId { got: 1, expected: 137 };
        assert_eq!(err.to_string(), "invalid chain id: got 1, expected 137");
    }

    #[test]
    fn test_invalid_signature_display() {
        let err = TxPoolError::InvalidSignature;
        assert_eq!(err.to_string(), "invalid signature");
    }

    #[test]
    fn test_decode_error_display() {
        let err = TxPoolError::DecodeError("invalid RLP encoding".to_string());
        assert_eq!(err.to_string(), "failed to decode transaction: invalid RLP encoding");
    }

    #[test]
    fn test_intrinsic_gas_too_low_display() {
        let err = TxPoolError::IntrinsicGasTooLow { limit: 21000, intrinsic: 53000 };
        assert_eq!(err.to_string(), "gas limit 21000 below intrinsic gas 53000");
    }

    #[test]
    fn test_already_exists_display() {
        let err = TxPoolError::AlreadyExists;
        assert_eq!(err.to_string(), "transaction already exists");
    }

    #[test]
    fn test_state_error_display() {
        let err = TxPoolError::StateError("database connection failed".to_string());
        assert_eq!(err.to_string(), "state error: database connection failed");
    }

    #[test]
    fn test_replacement_underpriced_display() {
        let err = TxPoolError::ReplacementUnderpriced;
        assert_eq!(err.to_string(), "replacement transaction underpriced");
    }

    #[test]
    fn test_txpool_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TxPoolError>();
    }

    #[test]
    fn test_txpool_error_debug() {
        let err = TxPoolError::NonceTooLow { got: 1, expected: 5 };
        let debug = format!("{:?}", err);
        assert!(debug.contains("NonceTooLow"));
        assert!(debug.contains("got: 1"));
        assert!(debug.contains("expected: 5"));
    }
}
