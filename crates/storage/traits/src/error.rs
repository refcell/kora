//! Error types for state database operations.

use alloy_primitives::B256;
use thiserror::Error;

/// Error type for state database operations.
#[derive(Debug, Error)]
pub enum StateDbError {
    /// Account not found.
    #[error("account not found: {0}")]
    AccountNotFound(alloy_primitives::Address),

    /// Code not found for hash.
    #[error("code not found: {0}")]
    CodeNotFound(B256),

    /// Storage error from underlying store.
    #[error("storage error: {0}")]
    Storage(String),

    /// Lock was poisoned.
    #[error("lock poisoned")]
    LockPoisoned,

    /// State root computation failed.
    #[error("root computation failed: {0}")]
    RootComputation(String),
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;

    use super::*;

    #[test]
    fn account_not_found_display() {
        let addr = Address::ZERO;
        let err = StateDbError::AccountNotFound(addr);
        assert_eq!(err.to_string(), format!("account not found: {addr}"));
    }

    #[test]
    fn code_not_found_display() {
        let hash = B256::ZERO;
        let err = StateDbError::CodeNotFound(hash);
        assert_eq!(err.to_string(), format!("code not found: {hash}"));
    }

    #[test]
    fn storage_error_display() {
        let err = StateDbError::Storage("disk full".to_string());
        assert_eq!(err.to_string(), "storage error: disk full");
    }

    #[test]
    fn lock_poisoned_display() {
        let err = StateDbError::LockPoisoned;
        assert_eq!(err.to_string(), "lock poisoned");
    }

    #[test]
    fn root_computation_display() {
        let err = StateDbError::RootComputation("invalid trie".to_string());
        assert_eq!(err.to_string(), "root computation failed: invalid trie");
    }

    #[test]
    fn error_debug_impl() {
        let err = StateDbError::LockPoisoned;
        assert!(format!("{:?}", err).contains("LockPoisoned"));
    }
}
