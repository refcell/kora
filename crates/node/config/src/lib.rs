//! Configuration types for Kora nodes.
#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod consensus;
pub use consensus::{ConsensusConfig, DEFAULT_THRESHOLD};

mod error;
pub use error::ConfigError;

mod execution;
pub use execution::{DEFAULT_BLOCK_TIME, DEFAULT_GAS_LIMIT, ExecutionConfig};

mod network;
pub use network::{DEFAULT_LISTEN_ADDR, NetworkConfig};

mod node;
pub use node::{DEFAULT_CHAIN_ID, DEFAULT_DATA_DIR, NodeConfig};

mod rpc;
pub use rpc::{DEFAULT_HTTP_ADDR, DEFAULT_WS_ADDR, RpcConfig};
