//! Block and transaction indexer for Kora RPC queries.

#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod error;
pub use error::IndexerError;

mod filter;
pub use filter::LogFilter;

mod store;
pub use store::BlockIndex;

mod types;
pub use types::{IndexStats, IndexedBlock, IndexedLog, IndexedReceipt, IndexedTransaction};
