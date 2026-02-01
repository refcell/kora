//! QMDB store ownership and state transitions.

use alloy_primitives::{Address, B256, U256};

use crate::{
    batch::StoreBatches,
    changes::ChangeSet,
    encoding::{AccountEncoding, StorageKey},
    error::QmdbError,
    traits::{QmdbBatchable, QmdbGettable},
};

/// The three QMDB stores.
#[derive(Debug)]
pub struct Stores<A, S, C> {
    /// Account store.
    pub accounts: A,
    /// Storage store.
    pub storage: S,
    /// Code store.
    pub code: C,
}

impl<A, S, C> Stores<A, S, C> {
    /// Create new stores.
    pub const fn new(accounts: A, storage: S, code: C) -> Self {
        Self { accounts, storage, code }
    }
}

/// Layer 1: Owns QMDB stores, handles state transitions.
///
/// NO synchronization - that's the caller's responsibility.
/// Use `kora-handlers::QmdbHandle` for thread-safe access.
#[derive(Debug)]
pub struct QmdbStore<A, S, C> {
    stores: Option<Stores<A, S, C>>,
}

impl<A, S, C> QmdbStore<A, S, C> {
    /// Create a new store from the three partitions.
    pub const fn new(accounts: A, storage: S, code: C) -> Self {
        Self { stores: Some(Stores::new(accounts, storage, code)) }
    }

    /// Borrow stores for reading.
    ///
    /// # Errors
    ///
    /// Returns [`QmdbError::StoreUnavailable`] if stores have been taken and not restored.
    pub fn stores(&self) -> Result<&Stores<A, S, C>, QmdbError> {
        self.stores.as_ref().ok_or(QmdbError::StoreUnavailable)
    }

    /// Mutably borrow stores.
    ///
    /// # Errors
    ///
    /// Returns [`QmdbError::StoreUnavailable`] if stores have been taken and not restored.
    pub fn stores_mut(&mut self) -> Result<&mut Stores<A, S, C>, QmdbError> {
        self.stores.as_mut().ok_or(QmdbError::StoreUnavailable)
    }

    /// Take ownership of stores for mutation.
    ///
    /// # Errors
    ///
    /// Returns [`QmdbError::StoreUnavailable`] if stores have been taken and not restored.
    pub fn take_stores(&mut self) -> Result<Stores<A, S, C>, QmdbError> {
        self.stores.take().ok_or(QmdbError::StoreUnavailable)
    }

    /// Restore stores after mutation.
    pub fn restore_stores(&mut self, stores: Stores<A, S, C>) {
        self.stores = Some(stores);
    }
}

impl<A, S, C> QmdbStore<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]>,
    S: QmdbGettable<Key = StorageKey, Value = U256>,
    C: QmdbGettable<Key = B256, Value = Vec<u8>>,
{
    /// Get account info.
    ///
    /// # Errors
    ///
    /// Returns an error if stores are unavailable, the account encoding is invalid,
    /// or the underlying storage operation fails.
    pub async fn get_account(
        &self,
        address: &Address,
    ) -> Result<Option<(u64, U256, B256, u64)>, QmdbError> {
        let stores = self.stores()?;
        match stores.accounts.get(address).await {
            Ok(Some(bytes)) => {
                AccountEncoding::decode(&bytes).ok_or(QmdbError::DecodeError).map(Some)
            }
            Ok(None) => Ok(None),
            Err(e) => Err(QmdbError::Storage(e.to_string())),
        }
    }

    /// Get storage value.
    ///
    /// # Errors
    ///
    /// Returns an error if stores are unavailable or the underlying storage operation fails.
    pub async fn get_storage(&self, key: &StorageKey) -> Result<Option<U256>, QmdbError> {
        let stores = self.stores()?;
        stores.storage.get(key).await.map_err(|e| QmdbError::Storage(e.to_string()))
    }

    /// Get code by hash.
    ///
    /// # Errors
    ///
    /// Returns an error if stores are unavailable or the underlying storage operation fails.
    pub async fn get_code(&self, hash: &B256) -> Result<Option<Vec<u8>>, QmdbError> {
        let stores = self.stores()?;
        stores.code.get(hash).await.map_err(|e| QmdbError::Storage(e.to_string()))
    }
}

impl<A, S, C> QmdbStore<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]>
        + QmdbBatchable<Key = Address, Value = [u8; AccountEncoding::SIZE]>,
    S: QmdbGettable<Key = StorageKey, Value = U256> + QmdbBatchable<Key = StorageKey, Value = U256>,
    C: QmdbGettable<Key = B256, Value = Vec<u8>> + QmdbBatchable<Key = B256, Value = Vec<u8>>,
{
    /// Build batches from a change set.
    ///
    /// # Errors
    ///
    /// Returns an error if stores are unavailable or the underlying storage operation fails.
    pub async fn build_batches(&self, changes: &ChangeSet) -> Result<StoreBatches, QmdbError> {
        let stores = self.stores()?;
        let mut batches = StoreBatches::new();

        for (address, update) in &changes.accounts {
            // Get current account to check generation
            let current_gen = match stores.accounts.get(address).await {
                Ok(Some(bytes)) => {
                    AccountEncoding::decode(&bytes).map(|(_, _, _, g)| g).unwrap_or(0)
                }
                Ok(None) => 0,
                Err(e) => return Err(QmdbError::Storage(e.to_string())),
            };

            // Increment generation on recreate or selfdestruct to invalidate old storage.
            let new_gen = if update.created || update.selfdestructed {
                current_gen.saturating_add(1)
            } else {
                current_gen
            };

            if update.selfdestructed {
                batches.accounts.push((*address, None));
            } else {
                let encoded = AccountEncoding::encode(
                    update.nonce,
                    update.balance,
                    update.code_hash,
                    new_gen,
                );
                batches.accounts.push((*address, Some(encoded)));

                // Add code if present
                if let Some(ref code) = update.code {
                    batches.code.push((update.code_hash, Some(code.clone())));
                }
            }

            // Add storage changes
            for (slot, value) in &update.storage {
                let key = StorageKey::new(*address, new_gen, *slot);
                if value.is_zero() {
                    batches.storage.push((key, None));
                } else {
                    batches.storage.push((key, Some(*value)));
                }
            }
        }

        Ok(batches)
    }

    /// Apply batches to stores.
    ///
    /// # Errors
    ///
    /// Returns an error if stores are unavailable or any batch write operation fails.
    pub async fn apply_batches(&mut self, batches: StoreBatches) -> Result<(), QmdbError> {
        let stores = self.stores_mut()?;

        stores
            .accounts
            .write_batch(batches.accounts)
            .await
            .map_err(|e| QmdbError::Storage(e.to_string()))?;

        stores
            .storage
            .write_batch(batches.storage)
            .await
            .map_err(|e| QmdbError::Storage(e.to_string()))?;

        stores
            .code
            .write_batch(batches.code)
            .await
            .map_err(|e| QmdbError::Storage(e.to_string()))?;

        Ok(())
    }

    /// Commit a change set to stores.
    ///
    /// # Errors
    ///
    /// Returns an error if stores are unavailable or any storage operation fails.
    pub async fn commit_changes(&mut self, changes: ChangeSet) -> Result<(), QmdbError> {
        if changes.is_empty() {
            return Ok(());
        }
        let batches = self.build_batches(&changes).await?;
        self.apply_batches(batches).await
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap as StdHashMap, sync::Mutex};

    use super::*;

    #[derive(Debug, Default)]
    struct MemoryStore<K, V> {
        data: Mutex<StdHashMap<K, V>>,
    }

    impl<K, V> MemoryStore<K, V> {
        fn new() -> Self {
            Self { data: Mutex::new(StdHashMap::new()) }
        }
    }

    #[derive(Debug)]
    struct MemoryError;

    impl std::fmt::Display for MemoryError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "memory error")
        }
    }

    impl std::error::Error for MemoryError {}

    impl<K: Clone + Eq + std::hash::Hash + Send + Sync, V: Clone + Send + Sync> QmdbGettable
        for MemoryStore<K, V>
    {
        type Error = MemoryError;
        type Key = K;
        type Value = V;

        async fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
            Ok(self.data.lock().unwrap().get(key).cloned())
        }
    }

    impl<K: Clone + Eq + std::hash::Hash + Send + Sync, V: Clone + Send + Sync> QmdbBatchable
        for MemoryStore<K, V>
    {
        async fn write_batch<I>(&mut self, ops: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = (Self::Key, Option<Self::Value>)> + Send,
            I::IntoIter: Send,
        {
            let mut data = self.data.lock().unwrap();
            for (key, value) in ops {
                match value {
                    Some(v) => {
                        data.insert(key, v);
                    }
                    None => {
                        data.remove(&key);
                    }
                }
            }
            Ok(())
        }
    }

    type TestStore = QmdbStore<
        MemoryStore<Address, [u8; 80]>,
        MemoryStore<StorageKey, U256>,
        MemoryStore<B256, Vec<u8>>,
    >;

    fn create_test_store() -> TestStore {
        QmdbStore::new(MemoryStore::new(), MemoryStore::new(), MemoryStore::new())
    }

    #[test]
    fn take_restore_pattern() {
        let mut store = create_test_store();
        let stores = store.take_stores().unwrap();
        assert!(store.stores().is_err());
        store.restore_stores(stores);
        assert!(store.stores().is_ok());
    }

    #[tokio::test]
    async fn commit_empty_changes() {
        let mut store = create_test_store();
        store.commit_changes(ChangeSet::new()).await.unwrap();
    }
}
