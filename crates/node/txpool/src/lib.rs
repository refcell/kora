//! Production-ready transaction pool for Kora nodes.
#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod config;
pub use config::PoolConfig;

mod error;
pub use error::TxPoolError;

mod ordering;
pub use ordering::{OrderedTransaction, SenderQueue};

mod traits;
pub use traits::Mempool;

mod pool;
pub use pool::TransactionPool;

mod validator;
pub use validator::{TransactionValidator, ValidatedTransaction, recover_sender_from_envelope};
