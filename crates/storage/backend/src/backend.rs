//! Commonware-based QMDB backend implementation.

use alloy_primitives::B256;
use async_trait::async_trait;
use commonware_codec::RangeCfg;
use commonware_cryptography::sha256::Digest as QmdbDigest;
use commonware_runtime::{Metrics as _, buffer::PoolRef};
use commonware_storage::{qmdb::any::VariableConfig, translator::EightCap};
use commonware_utils::{NZU64, NZUsize};
use kora_handlers::{HandleError, RootProvider};
use kora_qmdb::{ChangeSet, QmdbStore, StateRoot};

use crate::{
    AccountStore, BackendError, CodeStore, QmdbBackendConfig, StorageStore,
    accounts::AccountStoreDirty, code::CodeStoreDirty, storage::StorageStoreDirty, types::Context,
};

const CODE_MAX_BYTES: usize = 24_576;

/// Commonware-based QMDB backend.
///
/// Provides storage for accounts, storage slots, and code using
/// commonware-storage primitives.
pub struct CommonwareBackend {
    accounts: AccountStore,
    storage: StorageStore,
    code: CodeStore,
    context: Context,
    config: QmdbBackendConfig,
}

/// Root provider that computes state roots from commonware-storage partitions.
#[derive(Clone)]
pub struct CommonwareRootProvider {
    context: Context,
    config: QmdbBackendConfig,
}

impl std::fmt::Debug for CommonwareBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommonwareBackend").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for CommonwareRootProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommonwareRootProvider").finish_non_exhaustive()
    }
}

impl CommonwareRootProvider {
    /// Create a new root provider from the given context and config.
    #[must_use]
    pub const fn new(context: Context, config: QmdbBackendConfig) -> Self {
        Self { context, config }
    }
}

impl CommonwareBackend {
    /// Open a backend with the given configuration.
    pub async fn open(context: Context, config: QmdbBackendConfig) -> Result<Self, BackendError> {
        let stores = open_stores(context.clone(), &config).await?;
        Ok(Self {
            accounts: stores.accounts,
            storage: stores.storage,
            code: stores.code,
            context,
            config,
        })
    }

    /// Get a reference to the accounts store.
    #[must_use]
    pub const fn accounts(&self) -> &AccountStore {
        &self.accounts
    }

    /// Get a mutable reference to the accounts store.
    #[must_use]
    pub const fn accounts_mut(&mut self) -> &mut AccountStore {
        &mut self.accounts
    }

    /// Get a reference to the storage store.
    #[must_use]
    pub const fn storage(&self) -> &StorageStore {
        &self.storage
    }

    /// Get a mutable reference to the storage store.
    #[must_use]
    pub const fn storage_mut(&mut self) -> &mut StorageStore {
        &mut self.storage
    }

    /// Get a reference to the code store.
    #[must_use]
    pub const fn code(&self) -> &CodeStore {
        &self.code
    }

    /// Get a mutable reference to the code store.
    #[must_use]
    pub const fn code_mut(&mut self) -> &mut CodeStore {
        &mut self.code
    }

    /// Consume the backend and return the underlying stores.
    pub fn into_stores(self) -> (AccountStore, StorageStore, CodeStore) {
        (self.accounts, self.storage, self.code)
    }

    /// Build a root provider for this backend configuration.
    pub fn root_provider(&self) -> CommonwareRootProvider {
        CommonwareRootProvider::new(self.context.clone(), self.config.clone())
    }

    /// Get the current state root.
    pub fn state_root(&self) -> Result<B256, BackendError> {
        state_root_from_stores(&self.accounts, &self.storage, &self.code)
    }
}

#[async_trait]
impl RootProvider for CommonwareRootProvider {
    async fn state_root(&self) -> Result<B256, HandleError> {
        let stores = open_stores(self.context.clone(), &self.config)
            .await
            .map_err(|e| HandleError::RootComputation(e.to_string()))?;
        state_root_from_stores(&stores.accounts, &stores.storage, &stores.code)
            .map_err(|e| HandleError::RootComputation(e.to_string()))
    }

    async fn compute_root(&mut self, changes: &ChangeSet) -> Result<B256, HandleError> {
        if changes.is_empty() {
            return self.state_root().await;
        }

        let stores = open_dirty_stores(self.context.clone(), &self.config)
            .await
            .map_err(|e| HandleError::RootComputation(e.to_string()))?;
        let mut qmdb = QmdbStore::new(stores.accounts, stores.storage, stores.code);
        qmdb.commit_changes(changes.clone())
            .await
            .map_err(|e| HandleError::RootComputation(e.to_string()))?;
        let stores = qmdb.take_stores().map_err(|e| HandleError::RootComputation(e.to_string()))?;
        let accounts = stores.accounts.root();
        let storage = stores.storage.root();
        let code = stores.code.root();
        Ok(state_root_from_roots(accounts, storage, code))
    }

    async fn commit_and_get_root(&mut self) -> Result<B256, HandleError> {
        self.state_root().await
    }
}

struct Stores {
    accounts: AccountStore,
    storage: StorageStore,
    code: CodeStore,
}

struct DirtyStores {
    accounts: AccountStoreDirty,
    storage: StorageStoreDirty,
    code: CodeStoreDirty,
}

fn store_config<C>(
    prefix: &str,
    name: &str,
    buffer_pool: PoolRef,
    log_codec_config: C,
) -> VariableConfig<EightCap, C> {
    VariableConfig {
        mmr_journal_partition: format!("{prefix}-{name}-mmr"),
        mmr_metadata_partition: format!("{prefix}-{name}-mmr-meta"),
        mmr_items_per_blob: NZU64!(128),
        mmr_write_buffer: NZUsize!(1024 * 1024),
        log_partition: format!("{prefix}-{name}-log"),
        log_write_buffer: NZUsize!(1024 * 1024),
        log_compression: None,
        log_codec_config,
        log_items_per_blob: NZU64!(128),
        translator: EightCap,
        thread_pool: None,
        buffer_pool,
    }
}

async fn open_stores(context: Context, config: &QmdbBackendConfig) -> Result<Stores, BackendError> {
    let accounts = AccountStore::init(
        context.with_label("accounts"),
        store_config(&config.partition_prefix, "accounts", config.buffer_pool.clone(), ()),
    )
    .await
    .map_err(|e| BackendError::Storage(e.to_string()))?;

    let storage = StorageStore::init(
        context.with_label("storage"),
        store_config(&config.partition_prefix, "storage", config.buffer_pool.clone(), ()),
    )
    .await
    .map_err(|e| BackendError::Storage(e.to_string()))?;

    let code = CodeStore::init(
        context.with_label("code"),
        store_config(
            &config.partition_prefix,
            "code",
            config.buffer_pool.clone(),
            (RangeCfg::new(0..=CODE_MAX_BYTES), ()),
        ),
    )
    .await
    .map_err(|e| BackendError::Storage(e.to_string()))?;

    Ok(Stores { accounts, storage, code })
}

async fn open_dirty_stores(
    context: Context,
    config: &QmdbBackendConfig,
) -> Result<DirtyStores, BackendError> {
    let stores = open_stores(context, config).await?;
    Ok(DirtyStores {
        accounts: stores.accounts.into_dirty()?,
        storage: stores.storage.into_dirty()?,
        code: stores.code.into_dirty()?,
    })
}

fn state_root_from_stores(
    accounts: &AccountStore,
    storage: &StorageStore,
    code: &CodeStore,
) -> Result<B256, BackendError> {
    Ok(state_root_from_roots(accounts.root()?, storage.root()?, code.root()?))
}

fn state_root_from_roots(accounts: QmdbDigest, storage: QmdbDigest, code: QmdbDigest) -> B256 {
    StateRoot::compute(
        B256::from_slice(accounts.as_ref()),
        B256::from_slice(storage.as_ref()),
        B256::from_slice(code.as_ref()),
    )
}
