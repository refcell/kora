use std::sync::Arc;

use alloy_primitives::{Address, B256, U256};
use commonware_runtime::tokio::Context;
use kora_backend::{
    AccountStore, CodeStore, CommonwareBackend, CommonwareRootProvider, QmdbBackendConfig,
    StorageStore,
};
use kora_domain::StateRoot;
use kora_handlers::{HandleError, QmdbHandle, QmdbRefDb as HandlerQmdbRefDb};
use kora_qmdb::StateRoot as QmdbStateRoot;
use kora_traits::{StateDb, StateDbWrite};
use thiserror::Error;
use tokio::sync::RwLock;

/// QMDB configuration for the backend.
pub type QmdbConfig = QmdbBackendConfig;
/// QMDB change set type.
pub type QmdbChangeSet = kora_qmdb::ChangeSet;
/// QMDB handle type used as a state database.
pub type QmdbState = QmdbHandle<AccountStore, StorageStore, CodeStore>;
/// Tokio-backed REVM database wrapper for QMDB handles.
pub type QmdbRefDb = HandlerQmdbRefDb<AccountStore, StorageStore, CodeStore>;

type Handle = QmdbState;

/// QMDB ledger service backed by kora storage crates.
#[derive(Clone, Debug)]
pub struct QmdbLedger {
    handle: Handle,
}

/// Errors for QMDB ledger operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Backend error while opening QMDB storage.
    #[error("backend error: {0}")]
    Backend(#[from] kora_backend::BackendError),
    /// Handler error while applying state changes.
    #[error("handler error: {0}")]
    Handler(#[from] HandleError),
    /// State database error while computing or committing roots.
    #[error("state db error: {0}")]
    StateDb(#[from] kora_traits::StateDbError),
    /// Missing Tokio runtime needed for sync REVM database access.
    #[error("missing tokio runtime for async db bridge")]
    MissingRuntime,
}

impl QmdbLedger {
    /// Initializes the QMDB partitions and populates the genesis allocation.
    pub async fn init(
        context: Context,
        config: QmdbConfig,
        genesis_alloc: Vec<(Address, U256)>,
    ) -> Result<Self, Error> {
        let backend = CommonwareBackend::open(context.clone(), config.clone()).await?;
        let root_provider = CommonwareRootProvider::new(context, config);
        let (accounts, storage, code) = backend.into_stores();
        let handle = Handle::new(accounts, storage, code)
            .with_root_provider(Arc::new(RwLock::new(root_provider)));
        handle.init_genesis(genesis_alloc).await?;
        Ok(Self { handle })
    }

    /// Exposes a synchronous REVM database view backed by QMDB.
    pub fn database(&self) -> Result<QmdbRefDb, Error> {
        QmdbRefDb::new(self.handle.clone()).ok_or(Error::MissingRuntime)
    }

    /// Exposes the async state handle used by the block executor.
    pub fn state(&self) -> QmdbState {
        self.handle.clone()
    }

    /// Computes the root for a change set without committing.
    pub async fn compute_root(&self, changes: QmdbChangeSet) -> Result<StateRoot, Error> {
        let root = StateDbWrite::compute_root(&self.handle, &changes).await?;
        Ok(StateRoot(root))
    }

    /// Commits the provided changes to QMDB and returns the resulting root.
    pub async fn commit_changes(&self, changes: QmdbChangeSet) -> Result<StateRoot, Error> {
        let _storage_access = self.handle.storage_access().await;
        let mut store = self.handle.write().await;
        store
            .commit_changes(changes)
            .await
            .map_err(|e| kora_traits::StateDbError::Storage(e.to_string()))?;
        let stores =
            store.stores().map_err(|e| kora_traits::StateDbError::Storage(e.to_string()))?;
        let root = QmdbStateRoot::compute(
            B256::from_slice(stores.accounts.root()?.as_ref()),
            B256::from_slice(stores.storage.root()?.as_ref()),
            B256::from_slice(stores.code.root()?.as_ref()),
        );
        Ok(StateRoot(root))
    }

    /// Returns the current authenticated root stored in QMDB.
    pub async fn root(&self) -> Result<StateRoot, Error> {
        let root = StateDb::state_root(&self.handle).await?;
        Ok(StateRoot(root))
    }
}
