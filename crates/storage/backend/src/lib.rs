//! Concrete storage backend for Kora QMDB.

#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod accounts;
pub use accounts::{AccountStore, AccountStoreError};

mod types;

mod backend;
pub use backend::{CommonwareBackend, CommonwareRootProvider};

mod code;
pub use code::{CodeStore, CodeStoreError};

mod config;
pub use config::QmdbBackendConfig;

mod error;
pub use error::BackendError;

mod partition;
pub use partition::PartitionState;

mod storage;
pub use storage::{StorageStore, StorageStoreError};
