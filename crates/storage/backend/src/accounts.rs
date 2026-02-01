//! Account store bindings for commonware-storage.

use alloy_primitives::Address;
use commonware_cryptography::sha256::Digest as QmdbDigest;
use commonware_storage::{kv::Batchable as _, qmdb::any::VariableConfig, translator::EightCap};
use kora_qmdb::{AccountEncoding, QmdbBatchable, QmdbGettable};

use crate::{
    BackendError,
    types::{AccountDb, AccountDbDirty, AccountKey, AccountValue, Context, StoreSlot},
};

/// Account partition backed by commonware-storage.
///
/// Stores account state including nonce, balance, code hash, and generation number.
/// Each account is keyed by its 20-byte address and encoded as a fixed 80-byte value
/// using [`AccountEncoding`](kora_qmdb::AccountEncoding).
///
/// Implements [`QmdbGettable`] for reads and [`QmdbBatchable`] for batch writes.
/// All writes are atomic and update the authenticated Merkle root.
pub struct AccountStore {
    inner: StoreSlot<AccountDb>,
}

pub(crate) struct AccountStoreDirty {
    inner: AccountDbDirty,
}

impl AccountStore {
    /// Initialize the account store.
    pub async fn init(
        context: Context,
        config: VariableConfig<EightCap, ()>,
    ) -> Result<Self, BackendError> {
        let inner = AccountDb::init(context, config)
            .await
            .map_err(|e| BackendError::Storage(e.to_string()))?;
        Ok(Self { inner: StoreSlot::new(inner) })
    }

    /// Return the current authenticated root for the account partition.
    pub fn root(&self) -> Result<QmdbDigest, BackendError> {
        Ok(self.inner.get()?.root())
    }

    pub(crate) fn into_dirty(self) -> Result<AccountStoreDirty, BackendError> {
        let inner = self.inner.into_inner()?;
        Ok(AccountStoreDirty { inner: inner.into_mutable() })
    }
}

impl AccountStoreDirty {
    pub(crate) fn root(self) -> QmdbDigest {
        self.inner.into_merkleized().root()
    }
}

impl std::fmt::Debug for AccountStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountStore").finish_non_exhaustive()
    }
}

/// Error type for account store operations.
pub type AccountStoreError = BackendError;

const fn account_key(address: Address) -> AccountKey {
    AccountKey::new(address.into_array())
}

impl QmdbGettable for AccountStore {
    type Key = Address;
    type Value = [u8; AccountEncoding::SIZE];
    type Error = AccountStoreError;

    async fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
        let record = self
            .inner
            .get()?
            .get(&account_key(*key))
            .await
            .map_err(|e| BackendError::Storage(e.to_string()))?;
        Ok(record.map(|value| value.0))
    }
}

impl QmdbBatchable for AccountStore {
    async fn write_batch<I>(&mut self, ops: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = (Self::Key, Option<Self::Value>)> + Send,
        I::IntoIter: Send,
    {
        let inner = self.inner.take()?;
        let mut dirty = inner.into_mutable();
        let mapped =
            ops.into_iter().map(|(address, value)| (account_key(address), value.map(AccountValue)));
        dirty.write_batch(mapped).await.map_err(|e| BackendError::Storage(e.to_string()))?;
        let merkleized = dirty.into_merkleized();
        let (inner, _) =
            merkleized.commit(None).await.map_err(|e| BackendError::Storage(e.to_string()))?;
        self.inner.restore(inner);
        Ok(())
    }
}

impl QmdbGettable for AccountStoreDirty {
    type Key = Address;
    type Value = [u8; AccountEncoding::SIZE];
    type Error = AccountStoreError;

    async fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
        let record = self
            .inner
            .get(&account_key(*key))
            .await
            .map_err(|e| BackendError::Storage(e.to_string()))?;
        Ok(record.map(|value| value.0))
    }
}

impl QmdbBatchable for AccountStoreDirty {
    async fn write_batch<I>(&mut self, ops: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = (Self::Key, Option<Self::Value>)> + Send,
        I::IntoIter: Send,
    {
        let mapped =
            ops.into_iter().map(|(address, value)| (account_key(address), value.map(AccountValue)));
        self.inner.write_batch(mapped).await.map_err(|e| BackendError::Storage(e.to_string()))
    }
}
