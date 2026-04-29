//! REVM database trait implementations.
//!
//! Note: REVM's `DatabaseRef` trait is synchronous, so we use `futures::executor::block_on`
//! to bridge the async QMDB traits into the sync REVM interface. This is acceptable for
//! in-memory stores but may block the async runtime for I/O-bound stores.

use std::sync::Arc;

use alloy_primitives::{Address, B256, Bytes, KECCAK256_EMPTY, U256};
use kora_qmdb::{AccountEncoding, ChangeSet, QmdbBatchable, QmdbGettable, StorageKey};
use revm::{
    bytecode::Bytecode,
    database_interface::{
        DatabaseCommit, DatabaseRef,
        async_db::{DatabaseAsyncRef, WrapDatabaseAsync},
    },
    primitives::AddressMap,
    state::Account,
};

use crate::{error::HandleError, qmdb::QmdbHandle};

/// Tokio-backed REVM database wrapper for async QMDB handles.
///
/// This adapter uses `WrapDatabaseAsync` under the hood to satisfy REVM's sync `DatabaseRef`
/// trait while executing async reads on a Tokio runtime.
pub struct QmdbRefDb<A, S, C> {
    inner: Arc<WrapDatabaseAsync<QmdbHandle<A, S, C>>>,
}

impl<A, S, C> Clone for QmdbRefDb<A, S, C> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

impl<A, S, C> std::fmt::Debug for QmdbRefDb<A, S, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QmdbRefDb").finish()
    }
}

impl<A, S, C> QmdbRefDb<A, S, C> {
    /// Wraps a QMDB handle with the current Tokio runtime.
    ///
    /// Returns `None` if no multi-threaded runtime is available.
    pub fn new(handle: QmdbHandle<A, S, C>) -> Option<Self> {
        WrapDatabaseAsync::new(handle).map(|wrapped| Self { inner: Arc::new(wrapped) })
    }
}

impl<A, S, C> DatabaseRef for QmdbRefDb<A, S, C>
where
    QmdbHandle<A, S, C>: DatabaseAsyncRef<Error = HandleError>,
{
    type Error = HandleError;

    fn basic_ref(&self, address: Address) -> Result<Option<revm::state::AccountInfo>, Self::Error> {
        self.inner.basic_ref(address)
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.inner.code_by_hash_ref(code_hash)
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.inner.storage_ref(address, index)
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        self.inner.block_hash_ref(number)
    }
}

/// Wrapper for blocking async operations in sync contexts.
///
/// This is used to bridge async QMDB operations into REVM's sync DatabaseRef trait.
fn block_on<F: std::future::Future>(f: F) -> F::Output {
    futures::executor::block_on(f)
}

impl<A, S, C> DatabaseRef for QmdbHandle<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]>,
    S: QmdbGettable<Key = StorageKey, Value = U256>,
    C: QmdbGettable<Key = B256, Value = Vec<u8>>,
{
    type Error = HandleError;

    fn basic_ref(&self, address: Address) -> Result<Option<revm::state::AccountInfo>, Self::Error> {
        let store = block_on(self.read());
        match block_on(store.get_account(&address))? {
            Some((nonce, balance, code_hash, _gen)) => Ok(Some(revm::state::AccountInfo {
                nonce,
                balance,
                code_hash,
                code: None,
                account_id: None,
            })),
            None => Ok(None),
        }
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        if code_hash == KECCAK256_EMPTY || code_hash == B256::ZERO {
            return Ok(Bytecode::default());
        }
        let store = block_on(self.read());
        block_on(store.get_code(&code_hash))?.map_or_else(
            || Err(HandleError::CodeNotFound(code_hash)),
            |bytes| Ok(Bytecode::new_raw(Bytes::from(bytes))),
        )
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let store = block_on(self.read());

        // Get account to find generation
        let generation = match block_on(store.get_account(&address))? {
            Some((_, _, _, generation)) => generation,
            None => return Ok(U256::ZERO),
        };

        let key = StorageKey::new(address, generation, index);
        Ok(block_on(store.get_storage(&key))?.unwrap_or(U256::ZERO))
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        Err(HandleError::BlockHashNotFound(number))
    }
}

impl<A, S, C> DatabaseAsyncRef for QmdbHandle<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]> + Send + Sync + 'static,
    S: QmdbGettable<Key = StorageKey, Value = U256> + Send + Sync + 'static,
    C: QmdbGettable<Key = B256, Value = Vec<u8>> + Send + Sync + 'static,
{
    type Error = HandleError;

    fn basic_async_ref(
        &self,
        address: Address,
    ) -> impl std::future::Future<Output = Result<Option<revm::state::AccountInfo>, Self::Error>> + Send
    {
        let handle = self.clone();
        async move {
            let store = handle.read().await;
            match store.get_account(&address).await? {
                Some((nonce, balance, code_hash, _gen)) => Ok(Some(revm::state::AccountInfo {
                    nonce,
                    balance,
                    code_hash,
                    code: None,
                    account_id: None,
                })),
                None => Ok(None),
            }
        }
    }

    fn code_by_hash_async_ref(
        &self,
        code_hash: B256,
    ) -> impl std::future::Future<Output = Result<Bytecode, Self::Error>> + Send {
        let handle = self.clone();
        async move {
            if code_hash == KECCAK256_EMPTY || code_hash == B256::ZERO {
                return Ok(Bytecode::default());
            }
            let store = handle.read().await;
            store.get_code(&code_hash).await?.map_or_else(
                || Err(HandleError::CodeNotFound(code_hash)),
                |bytes| Ok(Bytecode::new_raw(Bytes::from(bytes))),
            )
        }
    }

    fn storage_async_ref(
        &self,
        address: Address,
        index: U256,
    ) -> impl std::future::Future<Output = Result<U256, Self::Error>> + Send {
        let handle = self.clone();
        async move {
            let store = handle.read().await;
            let generation = match store.get_account(&address).await? {
                Some((_, _, _, generation)) => generation,
                None => return Ok(U256::ZERO),
            };
            let key = StorageKey::new(address, generation, index);
            Ok(store.get_storage(&key).await?.unwrap_or(U256::ZERO))
        }
    }

    fn block_hash_async_ref(
        &self,
        number: u64,
    ) -> impl std::future::Future<Output = Result<B256, Self::Error>> + Send {
        std::future::ready(Err(HandleError::BlockHashNotFound(number)))
    }
}

impl<A, S, C> DatabaseCommit for QmdbHandle<A, S, C>
where
    A: QmdbGettable<Key = Address, Value = [u8; AccountEncoding::SIZE]>
        + QmdbBatchable<Key = Address, Value = [u8; AccountEncoding::SIZE]>,
    S: QmdbGettable<Key = StorageKey, Value = U256> + QmdbBatchable<Key = StorageKey, Value = U256>,
    C: QmdbGettable<Key = B256, Value = Vec<u8>> + QmdbBatchable<Key = B256, Value = Vec<u8>>,
{
    fn commit(&mut self, changes: AddressMap<Account>) {
        use std::collections::BTreeMap;

        use kora_qmdb::AccountUpdate;

        let mut changeset = ChangeSet::new();

        for (address, account) in changes {
            if !account.is_touched() {
                continue;
            }

            let storage: BTreeMap<U256, U256> =
                account.storage.iter().map(|(k, v)| (*k, v.present_value())).collect();

            let code = account.info.code.as_ref().map(|c| c.bytes().to_vec());

            changeset.accounts.insert(
                address,
                AccountUpdate {
                    created: account.is_created(),
                    selfdestructed: account.is_selfdestructed(),
                    nonce: account.info.nonce,
                    balance: account.info.balance,
                    code_hash: account.info.code_hash,
                    code,
                    storage,
                },
            );
        }

        // Ignore errors in DatabaseCommit (matches REVM's signature)
        let _ = block_on(Self::commit(self, changeset));
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap as StdHashMap, sync::Mutex};

    use kora_qmdb::{QmdbBatchable, QmdbGettable};
    use revm::database_interface::DatabaseRef;

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
    fn basic_ref_returns_none_for_missing() {
        let handle = create_test_handle();
        let result = handle.basic_ref(Address::ZERO).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn storage_ref_returns_zero_for_missing() {
        let handle = create_test_handle();
        let result = handle.storage_ref(Address::ZERO, U256::from(1)).unwrap();
        assert_eq!(result, U256::ZERO);
    }

    #[test]
    fn code_by_hash_returns_empty_for_keccak_empty() {
        let handle = create_test_handle();
        let result = handle.code_by_hash_ref(KECCAK256_EMPTY).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn block_hash_returns_error() {
        let handle = create_test_handle();
        let result = handle.block_hash_ref(100);
        assert!(matches!(result, Err(HandleError::BlockHashNotFound(100))));
    }
}
