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
