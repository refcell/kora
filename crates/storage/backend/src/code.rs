//! Code store bindings for commonware-storage.

use alloy_primitives::B256;
use commonware_cryptography::sha256::Digest as QmdbDigest;
use commonware_storage::{kv::Batchable as _, qmdb::any::VariableConfig, translator::EightCap};
use kora_qmdb::{QmdbBatchable, QmdbGettable};

use crate::{
    BackendError,
    types::{CodeDb, CodeDbDirty, CodeKey, Context, StoreSlot},
};

/// Code partition backed by commonware-storage.
///
/// Stores contract bytecode keyed by the keccak256 hash of the code (code hash).
/// Values are variable-length byte vectors containing the raw EVM bytecode.
///
/// Implements [`QmdbGettable`] for reads and [`QmdbBatchable`] for batch writes.
/// All writes are atomic and update the authenticated Merkle root.
pub struct CodeStore {
    inner: StoreSlot<CodeDb>,
}

pub(crate) struct CodeStoreDirty {
    inner: CodeDbDirty,
}

impl CodeStore {
    /// Initialize the code store.
    pub async fn init(
        context: Context,
        config: VariableConfig<EightCap, (commonware_codec::RangeCfg<usize>, ())>,
    ) -> Result<Self, BackendError> {
        let inner = CodeDb::init(context, config)
            .await
            .map_err(|e| BackendError::Storage(e.to_string()))?;
        Ok(Self { inner: StoreSlot::new(inner) })
    }

    /// Return the current authenticated root for the code partition.
    pub fn root(&self) -> Result<QmdbDigest, BackendError> {
        Ok(self.inner.get()?.root())
    }

    pub(crate) fn into_dirty(self) -> Result<CodeStoreDirty, BackendError> {
        let inner = self.inner.into_inner()?;
        Ok(CodeStoreDirty { inner: inner.into_mutable() })
    }
}

impl CodeStoreDirty {
    pub(crate) fn root(self) -> QmdbDigest {
        self.inner.into_merkleized().root()
    }
}

impl std::fmt::Debug for CodeStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeStore").finish_non_exhaustive()
    }
}

/// Error type for code store operations.
pub type CodeStoreError = BackendError;

const fn code_key(hash: B256) -> CodeKey {
    CodeKey::new(hash.0)
}

impl QmdbGettable for CodeStore {
    type Key = B256;
    type Value = Vec<u8>;
    type Error = CodeStoreError;

    async fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
        self.inner
            .get()?
            .get(&code_key(*key))
            .await
            .map_err(|e| BackendError::Storage(e.to_string()))
    }
}

impl QmdbBatchable for CodeStore {
    async fn write_batch<I>(&mut self, ops: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = (Self::Key, Option<Self::Value>)> + Send,
        I::IntoIter: Send,
    {
        let inner = self.inner.take()?;
        let mut dirty = inner.into_mutable();
        let mapped = ops.into_iter().map(|(hash, value)| (code_key(hash), value));
        dirty.write_batch(mapped).await.map_err(|e| BackendError::Storage(e.to_string()))?;
        let merkleized = dirty.into_merkleized();
        let (inner, _) =
            merkleized.commit(None).await.map_err(|e| BackendError::Storage(e.to_string()))?;
        self.inner.restore(inner);
        Ok(())
    }
}

impl QmdbGettable for CodeStoreDirty {
    type Key = B256;
    type Value = Vec<u8>;
    type Error = CodeStoreError;

    async fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
        self.inner.get(&code_key(*key)).await.map_err(|e| BackendError::Storage(e.to_string()))
    }
}

impl QmdbBatchable for CodeStoreDirty {
    async fn write_batch<I>(&mut self, ops: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = (Self::Key, Option<Self::Value>)> + Send,
        I::IntoIter: Send,
    {
        let mapped = ops.into_iter().map(|(hash, value)| (code_key(hash), value));
        self.inner.write_batch(mapped).await.map_err(|e| BackendError::Storage(e.to_string()))
    }
}
