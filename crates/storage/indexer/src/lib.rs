//! Block and transaction indexer for Kora RPC queries.
//!
//! This crate provides in-memory indexing for blocks, transactions, receipts,
//! and logs to support Ethereum JSON-RPC queries such as:
//! - `eth_getBlockByNumber` / `eth_getBlockByHash`
//! - `eth_getTransactionByHash`
//! - `eth_getTransactionReceipt`
//! - `eth_getLogs`

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
pub use types::{IndexedBlock, IndexedLog, IndexedReceipt, IndexedTransaction};
