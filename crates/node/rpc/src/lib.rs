//! RPC server for Kora nodes.
//!
//! Provides HTTP endpoints for querying node status and chain state,
//! as well as a full Ethereum JSON-RPC 2.0 API implementation.
//!
//! # Features
//!
//! - HTTP status and health endpoints via axum
//! - Ethereum JSON-RPC 2.0 API via jsonrpsee
//! - Core eth_* methods for wallet and dApp compatibility
//! - net_* and web3_* namespace support
//! - Configurable CORS and rate limiting for production use
//! - Pluggable state provider for ledger integration
//!
//! # Example
//!
//! ```ignore
//! use kora_rpc::{RpcServer, NodeState, NoopStateProvider};
//! use std::net::SocketAddr;
//!
//! let state = NodeState::new(1, 0);
//! let addr: SocketAddr = "127.0.0.1:8545".parse().unwrap();
//! let server = RpcServer::with_chain_id(state, addr, 1337);
//! let handle = server.start();
//! ```

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod config;
pub use config::{CorsConfig, RateLimitConfig, RpcServerConfig};

mod error;
pub use error::{codes as error_codes, RpcError};

mod eth;
pub use eth::{
    EthApiImpl, EthApiServer, FeeHistory, NetApiImpl, NetApiServer, TxSubmitCallback,
    Web3ApiImpl, Web3ApiServer,
};

mod kora;
pub use kora::{KoraApiImpl, KoraApiServer};

mod server;
pub use server::{JsonRpcServer, RpcServer, RpcServerHandle, ServerError};

mod state;
pub use state::{NodeState, NodeStatus};

mod state_provider;
pub use state_provider::{NoopStateProvider, StateProvider};

mod indexed_provider;
pub use indexed_provider::IndexedStateProvider;

mod types;
pub use types::{
    BlockNumberOrTag, BlockTag, BlockTransactions, CallRequest, RpcBlock, RpcLog,
    RpcTransaction, RpcTransactionReceipt, SyncInfo, SyncStatus,
};
