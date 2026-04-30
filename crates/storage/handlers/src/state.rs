//! StateDb trait implementations for QmdbHandle.

use alloy_primitives::{Address, B256, Bytes, KECCAK256_EMPTY, U256};
use kora_qmdb::{AccountEncoding, ChangeSet, QmdbBatchable, QmdbGettable, StateRoot, StorageKey};
use kora_traits::{StateDb, StateDbError, StateDbRead, StateDbWrite};

use crate::QmdbHandle;

impl<A, S, C> StateDbRead for QmdbHandle<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]> + Send + Sync + 'static,
    S: QmdbGettable<Key = StorageKey, Value = U256> + Send + Sync + 'static,
    C: QmdbGettable<Key = B256, Value = Vec<u8>> + Send + Sync + 'static,
{
    async fn nonce(&self, address: &Address) -> Result<u64, StateDbError> {
        let store = self.read().await;
        match store.get_account(address).await.map_err(|e| StateDbError::Storage(e.to_string()))? {
            Some((nonce, _, _, _)) => Ok(nonce),
            None => Err(StateDbError::AccountNotFound(*address)),
        }
    }

    async fn balance(&self, address: &Address) -> Result<U256, StateDbError> {
        let store = self.read().await;
        match store.get_account(address).await.map_err(|e| StateDbError::Storage(e.to_string()))? {
            Some((_, balance, _, _)) => Ok(balance),
            None => Err(StateDbError::AccountNotFound(*address)),
        }
    }

    async fn code_hash(&self, address: &Address) -> Result<B256, StateDbError> {
        let store = self.read().await;
        match store.get_account(address).await.map_err(|e| StateDbError::Storage(e.to_string()))? {
            Some((_, _, code_hash, _)) => Ok(code_hash),
            None => Err(StateDbError::AccountNotFound(*address)),
        }
    }

    async fn code(&self, code_hash: &B256) -> Result<Bytes, StateDbError> {
        if *code_hash == KECCAK256_EMPTY || *code_hash == B256::ZERO {
            return Ok(Bytes::new());
        }
        let store = self.read().await;
        store
            .get_code(code_hash)
            .await
            .map_err(|e| StateDbError::Storage(e.to_string()))?
            .map_or_else(
                || Err(StateDbError::CodeNotFound(*code_hash)),
                |bytes| Ok(Bytes::from(bytes)),
            )
    }

    async fn storage(&self, address: &Address, slot: &U256) -> Result<U256, StateDbError> {
        let store = self.read().await;

        // Get account to find generation
        let generation = match store
            .get_account(address)
            .await
            .map_err(|e| StateDbError::Storage(e.to_string()))?
        {
            Some((_, _, _, generation)) => generation,
            None => return Ok(U256::ZERO),
        };

        let key = StorageKey::new(*address, generation, *slot);
        Ok(store
            .get_storage(&key)
            .await
            .map_err(|e| StateDbError::Storage(e.to_string()))?
            .unwrap_or(U256::ZERO))
    }
}

impl<A, S, C> StateDbWrite for QmdbHandle<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]>
        + QmdbBatchable<Key = Address, Value = [u8; AccountEncoding::SIZE]>
        + Send
        + Sync
        + 'static,
    S: QmdbGettable<Key = StorageKey, Value = U256>
        + QmdbBatchable<Key = StorageKey, Value = U256>
        + Send
        + Sync
        + 'static,
    C: QmdbGettable<Key = B256, Value = Vec<u8>>
        + QmdbBatchable<Key = B256, Value = Vec<u8>>
        + Send
        + Sync
        + 'static,
{
    async fn commit(&self, changes: ChangeSet) -> Result<B256, StateDbError> {
        let _storage_access = self.storage_access().await;
        let mut store = self.write().await;
        store.commit_changes(changes).await.map_err(|e| StateDbError::Storage(e.to_string()))?;
        drop(store);

        if let Some(provider) = self.root_provider() {
            let mut provider = provider.write().await;
            provider
                .commit_and_get_root()
                .await
                .map_err(|e| StateDbError::RootComputation(e.to_string()))
        } else {
            // Return placeholder root when no provider is set
            Ok(B256::ZERO)
        }
    }

    async fn compute_root(&self, changes: &ChangeSet) -> Result<B256, StateDbError> {
        // If we have a root provider, use it to compute the root
        if let Some(provider) = self.root_provider() {
            let _storage_access = self.storage_access().await;
            let mut provider = provider.write().await;
            provider
                .compute_root(changes)
                .await
                .map_err(|e| StateDbError::RootComputation(e.to_string()))
        } else {
            // Return placeholder root when no provider is set
            Ok(StateRoot::compute(B256::ZERO, B256::ZERO, B256::ZERO))
        }
    }

    fn merge_changes(&self, mut older: ChangeSet, newer: ChangeSet) -> ChangeSet {
        older.merge(newer);
        older
    }
}

impl<A, S, C> StateDb for QmdbHandle<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]>
        + QmdbBatchable<Key = Address, Value = [u8; AccountEncoding::SIZE]>
        + Send
        + Sync
        + 'static,
    S: QmdbGettable<Key = StorageKey, Value = U256>
        + QmdbBatchable<Key = StorageKey, Value = U256>
        + Send
        + Sync
        + 'static,
    C: QmdbGettable<Key = B256, Value = Vec<u8>>
        + QmdbBatchable<Key = B256, Value = Vec<u8>>
        + Send
        + Sync
        + 'static,
{
    async fn state_root(&self) -> Result<B256, StateDbError> {
        // If we have a root provider, use it to get the state root
        if let Some(provider) = self.root_provider() {
            let _storage_access = self.storage_access().await;
            let provider = provider.read().await;
            provider.state_root().await.map_err(|e| StateDbError::RootComputation(e.to_string()))
        } else {
            // Return zero root when no provider is set
            Ok(B256::ZERO)
        }
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

    #[tokio::test]
    async fn state_db_returns_error_for_missing_account() {
        let handle = create_test_handle();
        let result = handle.nonce(&Address::ZERO).await;
        assert!(matches!(result, Err(StateDbError::AccountNotFound(_))));
    }

    #[tokio::test]
    async fn state_db_returns_zero_for_missing_storage() {
        let handle = create_test_handle();
        let result = handle.storage(&Address::ZERO, &U256::from(1)).await.unwrap();
        assert_eq!(result, U256::ZERO);
    }

    #[tokio::test]
    async fn state_db_returns_empty_for_empty_code() {
        let handle = create_test_handle();
        let result = handle.code(&KECCAK256_EMPTY).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn state_db_merge_changes() {
        let handle = create_test_handle();
        let older = ChangeSet::new();
        let newer = ChangeSet::new();
        let merged = handle.merge_changes(older, newer);
        assert!(merged.is_empty());
    }
}
