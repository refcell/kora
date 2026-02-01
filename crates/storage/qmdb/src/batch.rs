//! Batch operations for QMDB writes.

use alloy_primitives::{Address, B256, U256};

use crate::encoding::StorageKey;

/// Batched operations ready for QMDB writes.
#[derive(Debug, Default)]
pub struct StoreBatches {
    /// Account operations: (address, encoded_account or None for deletion).
    pub accounts: Vec<(Address, Option<[u8; 80]>)>,
    /// Storage operations: (key, value or None for deletion).
    pub storage: Vec<(StorageKey, Option<U256>)>,
    /// Code operations: (hash, bytes or None for deletion).
    pub code: Vec<(B256, Option<Vec<u8>>)>,
}

impl StoreBatches {
    /// Create empty batches.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if all batches are empty.
    pub const fn is_empty(&self) -> bool {
        self.accounts.is_empty() && self.storage.is_empty() && self.code.is_empty()
    }

    /// Total number of operations across all batches.
    pub const fn len(&self) -> usize {
        self.accounts.len() + self.storage.len() + self.code.len()
    }
}
