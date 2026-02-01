//! Thread-safe QMDB handle.

use std::sync::Arc;

use alloy_primitives::{Address, B256, U256};
use async_trait::async_trait;
use kora_qmdb::{
    AccountEncoding, AccountUpdate, ChangeSet, QmdbBatchable, QmdbGettable, QmdbStore, StorageKey,
};
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::error::HandleError;

/// Trait for providing state root computation.
///
/// This trait abstracts the ability to compute and retrieve state roots
/// from a backend storage implementation.
#[async_trait]
pub trait RootProvider: Send + Sync {
    /// Get the current state root.
    async fn state_root(&self) -> Result<B256, HandleError>;

    /// Compute the state root without committing the provided changes.
    async fn compute_root(&mut self, changes: &ChangeSet) -> Result<B256, HandleError>;

    /// Commit changes and return the new state root.
    async fn commit_and_get_root(&mut self) -> Result<B256, HandleError>;
}

/// Thread-safe handle to QMDB stores.
///
/// Wraps `QmdbStore` with `Arc<RwLock>` for safe concurrent access.
/// Implements REVM database traits via the `adapter` module.
pub struct QmdbHandle<A, S, C> {
    inner: Arc<RwLock<QmdbStore<A, S, C>>>,
    root_provider: Option<Arc<RwLock<dyn RootProvider>>>,
}

impl<A, S, C> Clone for QmdbHandle<A, S, C> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner), root_provider: self.root_provider.clone() }
    }
}

impl<A, S, C> QmdbHandle<A, S, C> {
    /// Create a new handle from stores.
    #[must_use]
    pub fn new(accounts: A, storage: S, code: C) -> Self {
        Self {
            inner: Arc::new(RwLock::new(QmdbStore::new(accounts, storage, code))),
            root_provider: None,
        }
    }

    /// Create from an existing `QmdbStore`.
    #[must_use]
    pub fn from_store(store: QmdbStore<A, S, C>) -> Self {
        Self { inner: Arc::new(RwLock::new(store)), root_provider: None }
    }

    /// Set the root provider for state root computation.
    #[must_use]
    pub fn with_root_provider(mut self, provider: Arc<RwLock<dyn RootProvider>>) -> Self {
        self.root_provider = Some(provider);
        self
    }

    /// Get a reference to the root provider if set.
    pub fn root_provider(&self) -> Option<&Arc<RwLock<dyn RootProvider>>> {
        self.root_provider.as_ref()
    }

    /// Acquire read lock on the underlying store.
    pub async fn read(&self) -> RwLockReadGuard<'_, QmdbStore<A, S, C>> {
        self.inner.read().await
    }

    /// Acquire write lock on the underlying store.
    pub async fn write(&self) -> RwLockWriteGuard<'_, QmdbStore<A, S, C>> {
        self.inner.write().await
    }
}

impl<A, S, C> QmdbHandle<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]>
        + QmdbBatchable<Key = Address, Value = [u8; AccountEncoding::SIZE]>,
    S: QmdbGettable<Key = StorageKey, Value = U256> + QmdbBatchable<Key = StorageKey, Value = U256>,
    C: QmdbGettable<Key = B256, Value = Vec<u8>> + QmdbBatchable<Key = B256, Value = Vec<u8>>,
{
    /// Commit changes atomically.
    pub async fn commit(&self, changes: ChangeSet) -> Result<(), HandleError> {
        let mut store = self.write().await;
        store.commit_changes(changes).await?;
        Ok(())
    }

    /// Initialize with genesis allocations.
    pub async fn init_genesis(&self, allocs: Vec<(Address, U256)>) -> Result<(), HandleError> {
        use std::collections::BTreeMap;

        use alloy_primitives::KECCAK256_EMPTY;

        let mut changes = ChangeSet::new();
        for (address, balance) in allocs {
            changes.accounts.insert(
                address,
                AccountUpdate {
                    created: true,
                    selfdestructed: false,
                    nonce: 0,
                    balance,
                    code_hash: KECCAK256_EMPTY,
                    code: None,
                    storage: BTreeMap::new(),
                },
            );
        }
        self.commit(changes).await
    }
}

impl<A, S, C> std::fmt::Debug for QmdbHandle<A, S, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QmdbHandle").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap as StdHashMap, sync::Mutex};

    use kora_qmdb::{QmdbBatchable, QmdbGettable};

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

    type TestHandle = QmdbHandle<
        MemoryStore<Address, [u8; 80]>,
        MemoryStore<StorageKey, U256>,
        MemoryStore<B256, Vec<u8>>,
    >;

    fn create_test_handle() -> TestHandle {
        QmdbHandle::new(MemoryStore::new(), MemoryStore::new(), MemoryStore::new())
    }

    #[test]
    #[allow(clippy::redundant_clone)]
    fn handle_is_clone() {
        let handle = create_test_handle();
        let _cloned = handle.clone();
    }

    #[tokio::test]
    async fn init_genesis_creates_accounts() {
        let handle = create_test_handle();
        let allocs = vec![
            (Address::repeat_byte(0x01), U256::from(1000)),
            (Address::repeat_byte(0x02), U256::from(2000)),
        ];
        handle.init_genesis(allocs).await.unwrap();

        let store = handle.read().await;
        let acc1 = store.get_account(&Address::repeat_byte(0x01)).await.unwrap().unwrap();
        assert_eq!(acc1.1, U256::from(1000));

        let acc2 = store.get_account(&Address::repeat_byte(0x02)).await.unwrap().unwrap();
        assert_eq!(acc2.1, U256::from(2000));
    }
}
