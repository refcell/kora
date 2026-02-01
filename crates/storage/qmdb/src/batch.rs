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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_empty_batches() {
        let batches = StoreBatches::new();
        assert!(batches.is_empty());
        assert_eq!(batches.len(), 0);
    }

    #[test]
    fn test_default_creates_empty_batches() {
        let batches = StoreBatches::default();
        assert!(batches.is_empty());
        assert_eq!(batches.len(), 0);
    }

    #[test]
    fn test_is_empty_with_accounts() {
        let mut batches = StoreBatches::new();
        batches.accounts.push((Address::ZERO, Some([0u8; 80])));
        assert!(!batches.is_empty());
    }

    #[test]
    fn test_is_empty_with_storage() {
        let mut batches = StoreBatches::new();
        let key = StorageKey::new(Address::ZERO, 0, U256::ZERO);
        batches.storage.push((key, Some(U256::from(100))));
        assert!(!batches.is_empty());
    }

    #[test]
    fn test_is_empty_with_code() {
        let mut batches = StoreBatches::new();
        batches.code.push((B256::ZERO, Some(vec![0x60, 0x00])));
        assert!(!batches.is_empty());
    }

    #[test]
    fn test_len_counts_all_operations() {
        let mut batches = StoreBatches::new();

        batches.accounts.push((Address::ZERO, Some([0u8; 80])));
        batches.accounts.push((Address::repeat_byte(0x01), None));

        let key1 = StorageKey::new(Address::ZERO, 0, U256::from(1));
        let key2 = StorageKey::new(Address::ZERO, 0, U256::from(2));
        let key3 = StorageKey::new(Address::ZERO, 0, U256::from(3));
        batches.storage.push((key1, Some(U256::from(100))));
        batches.storage.push((key2, Some(U256::from(200))));
        batches.storage.push((key3, None));

        batches.code.push((B256::ZERO, Some(vec![0x60, 0x00])));

        assert_eq!(batches.len(), 6);
    }

    #[test]
    fn test_deletion_operations() {
        let mut batches = StoreBatches::new();

        batches.accounts.push((Address::ZERO, None));
        let key = StorageKey::new(Address::ZERO, 0, U256::ZERO);
        batches.storage.push((key, None));
        batches.code.push((B256::ZERO, None));

        assert!(!batches.is_empty());
        assert_eq!(batches.len(), 3);
    }

    #[test]
    fn test_debug_impl() {
        let batches = StoreBatches::new();
        let debug_str = format!("{:?}", batches);
        assert!(debug_str.contains("StoreBatches"));
    }
}
