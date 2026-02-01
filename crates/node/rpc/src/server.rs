//! HTTP and JSON-RPC server implementation.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use jsonrpsee::server::{Server, ServerHandle};
use tower::limit::ConcurrencyLimitLayer;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::{error, info};

use crate::{
    config::{CorsConfig, RpcServerConfig},
    eth::{
        EthApiImpl, EthApiServer, NetApiImpl, NetApiServer, TxSubmitCallback, Web3ApiImpl,
        Web3ApiServer,
    },
    kora::{KoraApiImpl, KoraApiServer},
    state::NodeState,
    state_provider::{NoopStateProvider, StateProvider},
};

/// Error type for RPC server operations.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// Failed to bind server.
    #[error("failed to bind server: {0}")]
    Bind(std::io::Error),
    /// Failed to build server.
    #[error("failed to build server: {0}")]
    Build(String),
    /// Failed to register RPC methods.
    #[error("failed to register RPC methods: {0}")]
    RegisterMethod(#[from] jsonrpsee::core::RegisterMethodError),
}

/// Build a CORS layer from configuration.
fn build_cors_layer(config: &CorsConfig) -> CorsLayer {
    if config.allowed_origins.is_empty() {
        return CorsLayer::new();
    }

    let mut layer = CorsLayer::new();

    if config.allowed_origins.len() == 1 && config.allowed_origins[0] == "*" {
        layer = layer.allow_origin(Any);
    } else {
        let origins: Vec<_> =
            config.allowed_origins.iter().filter_map(|o| o.parse().ok()).collect();
        layer = layer.allow_origin(AllowOrigin::list(origins));
    }

    if config.allowed_methods.iter().any(|m| m == "*") {
        layer = layer.allow_methods(Any);
    } else {
        let methods: Vec<_> =
            config.allowed_methods.iter().filter_map(|m| m.parse().ok()).collect();
        layer = layer.allow_methods(methods);
    }

    if config.allowed_headers.iter().any(|h| h == "*") {
        layer = layer.allow_headers(Any);
    } else {
        let headers: Vec<_> =
            config.allowed_headers.iter().filter_map(|h| h.parse().ok()).collect();
        layer = layer.allow_headers(headers);
    }

    layer.max_age(Duration::from_secs(config.max_age))
}

/// RPC server for exposing node status via HTTP and Ethereum JSON-RPC.
pub struct RpcServer<S: StateProvider = NoopStateProvider> {
    state: NodeState,
    addr: SocketAddr,
    chain_id: u64,
    tx_submit: Option<TxSubmitCallback>,
    state_provider: S,
    cors_config: CorsConfig,
    max_connections: u32,
}

impl<S: StateProvider> std::fmt::Debug for RpcServer<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcServer")
            .field("state", &self.state)
            .field("addr", &self.addr)
            .field("chain_id", &self.chain_id)
            .field("tx_submit", &self.tx_submit.is_some())
            .finish()
    }
}

impl RpcServer<NoopStateProvider> {
    /// Create a new RPC server with default (noop) state provider.
    pub fn new(state: NodeState, addr: SocketAddr) -> Self {
        Self {
            state,
            addr,
            chain_id: 1,
            tx_submit: None,
            state_provider: NoopStateProvider,
            cors_config: CorsConfig::default(),
            max_connections: 100,
        }
    }

    /// Create a new RPC server with chain ID.
    pub fn with_chain_id(state: NodeState, addr: SocketAddr, chain_id: u64) -> Self {
        Self {
            state,
            addr,
            chain_id,
            tx_submit: None,
            state_provider: NoopStateProvider,
            cors_config: CorsConfig::default(),
            max_connections: 100,
        }
    }
}

impl<S: StateProvider + Clone + 'static> RpcServer<S> {
    /// Create a new RPC server with a custom state provider.
    pub fn with_state_provider(
        state: NodeState,
        addr: SocketAddr,
        chain_id: u64,
        state_provider: S,
    ) -> Self {
        Self {
            state,
            addr,
            chain_id,
            tx_submit: None,
            state_provider,
            cors_config: CorsConfig::default(),
            max_connections: 100,
        }
    }

    /// Set the transaction submission callback.
    #[must_use]
    pub fn with_tx_submit(mut self, tx_submit: TxSubmitCallback) -> Self {
        self.tx_submit = Some(tx_submit);
        self
    }

    /// Set CORS configuration.
    #[must_use]
    pub fn with_cors(mut self, cors_config: CorsConfig) -> Self {
        self.cors_config = cors_config;
        self
    }

    /// Set maximum concurrent connections.
    #[must_use]
    pub const fn with_max_connections(mut self, max_connections: u32) -> Self {
        self.max_connections = max_connections;
        self
    }

    /// Create from configuration.
    pub fn from_config(state: NodeState, config: RpcServerConfig, state_provider: S) -> Self {
        Self {
            state,
            addr: config.http_addr,
            chain_id: config.chain_id,
            tx_submit: None,
            state_provider,
            cors_config: config.cors,
            max_connections: config.max_connections,
        }
    }

    /// Start the RPC server.
    ///
    /// This spawns background tasks for both HTTP and JSON-RPC servers and returns immediately.
    pub fn start(self) -> RpcServerHandle {
        let http_addr = self.addr;
        let jsonrpc_addr = self.addr;
        let node_state = Arc::new(self.state);
        let node_state_for_jsonrpc = Arc::clone(&node_state);
        let chain_id = self.chain_id;
        let tx_submit = self.tx_submit;
        let cors_layer = build_cors_layer(&self.cors_config);
        let max_connections = self.max_connections;
        let state_provider = self.state_provider;

        let http_handle = tokio::spawn(async move {
            let app = Router::new()
                .route("/status", get(status_handler))
                .route("/health", get(health_handler))
                .layer(cors_layer)
                .layer(ConcurrencyLimitLayer::new(max_connections as usize))
                .with_state(node_state);

            info!(addr = %http_addr, "Starting HTTP server");

            let listener = match tokio::net::TcpListener::bind(http_addr).await {
                Ok(l) => l,
                Err(e) => {
                    error!(error = %e, "Failed to bind HTTP server");
                    return;
                }
            };

            if let Err(e) = axum::serve(listener, app).await {
                error!(error = %e, "HTTP server error");
            }
        });

        let jsonrpc_handle = tokio::spawn(async move {
            let server = match Server::builder()
                .max_connections(max_connections)
                .build(jsonrpc_addr)
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    error!(error = %e, "Failed to build JSON-RPC server");
                    return None;
                }
            };

            let eth_api = tx_submit.map_or_else(
                || EthApiImpl::new(chain_id, state_provider.clone()),
                |submit| EthApiImpl::with_tx_submit(chain_id, state_provider.clone(), submit),
            );
            let net_api = NetApiImpl::new(chain_id);
            let web3_api = Web3ApiImpl::new();
            let kora_api = KoraApiImpl::new(node_state_for_jsonrpc);

            let mut module = jsonrpsee::RpcModule::new(());
            if let Err(e) = module.merge(eth_api.into_rpc()) {
                error!(error = %e, "Failed to merge eth API");
                return None;
            }
            if let Err(e) = module.merge(net_api.into_rpc()) {
                error!(error = %e, "Failed to merge net API");
                return None;
            }
            if let Err(e) = module.merge(web3_api.into_rpc()) {
                error!(error = %e, "Failed to merge web3 API");
                return None;
            }
            if let Err(e) = module.merge(kora_api.into_rpc()) {
                error!(error = %e, "Failed to merge kora API");
                return None;
            }

            info!(addr = %jsonrpc_addr, "Starting JSON-RPC server");

            let handle = server.start(module);
            handle.stopped().await;
            Some(())
        });

        RpcServerHandle { http_handle, jsonrpc_handle }
    }
}

/// Handle for managing the RPC server lifecycle.
pub struct RpcServerHandle {
    http_handle: tokio::task::JoinHandle<()>,
    jsonrpc_handle: tokio::task::JoinHandle<Option<()>>,
}

impl std::fmt::Debug for RpcServerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcServerHandle").finish_non_exhaustive()
    }
}

impl RpcServerHandle {
    /// Wait for both servers to complete.
    pub async fn stopped(self) {
        let _ = tokio::join!(self.http_handle, self.jsonrpc_handle);
    }

    /// Abort both servers.
    pub fn abort(self) {
        self.http_handle.abort();
        self.jsonrpc_handle.abort();
    }
}

async fn status_handler(State(state): State<Arc<NodeState>>) -> impl IntoResponse {
    let status = state.status();
    (StatusCode::OK, axum::Json(status))
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// Standalone JSON-RPC server without HTTP status endpoints.
pub struct JsonRpcServer<S: StateProvider = NoopStateProvider> {
    addr: SocketAddr,
    chain_id: u64,
    tx_submit: Option<TxSubmitCallback>,
    state_provider: S,
    max_connections: u32,
}

impl<S: StateProvider> std::fmt::Debug for JsonRpcServer<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsonRpcServer")
            .field("addr", &self.addr)
            .field("chain_id", &self.chain_id)
            .field("tx_submit", &self.tx_submit.is_some())
            .finish()
    }
}

impl JsonRpcServer<NoopStateProvider> {
    /// Create a new JSON-RPC server with default (noop) state provider.
    pub fn new(addr: SocketAddr, chain_id: u64) -> Self {
        Self {
            addr,
            chain_id,
            tx_submit: None,
            state_provider: NoopStateProvider,
            max_connections: 100,
        }
    }
}

impl<S: StateProvider + Clone + 'static> JsonRpcServer<S> {
    /// Create a new JSON-RPC server with a custom state provider.
    pub fn with_state_provider(addr: SocketAddr, chain_id: u64, state_provider: S) -> Self {
        Self { addr, chain_id, tx_submit: None, state_provider, max_connections: 100 }
    }

    /// Set the transaction submission callback.
    #[must_use]
    pub fn with_tx_submit(mut self, tx_submit: TxSubmitCallback) -> Self {
        self.tx_submit = Some(tx_submit);
        self
    }

    /// Set maximum concurrent connections.
    #[must_use]
    pub const fn with_max_connections(mut self, max_connections: u32) -> Self {
        self.max_connections = max_connections;
        self
    }

    /// Start the JSON-RPC server.
    pub async fn start(self) -> Result<ServerHandle, ServerError> {
        let server = Server::builder()
            .max_connections(self.max_connections)
            .build(self.addr)
            .await
            .map_err(|e| ServerError::Build(e.to_string()))?;

        let eth_api = self.tx_submit.map_or_else(
            || EthApiImpl::new(self.chain_id, self.state_provider.clone()),
            |submit| EthApiImpl::with_tx_submit(self.chain_id, self.state_provider.clone(), submit),
        );
        let net_api = NetApiImpl::new(self.chain_id);
        let web3_api = Web3ApiImpl::new();

        let mut module = jsonrpsee::RpcModule::new(());
        module.merge(eth_api.into_rpc())?;
        module.merge(net_api.into_rpc())?;
        module.merge(web3_api.into_rpc())?;

        info!(addr = %self.addr, "Starting JSON-RPC server");

        Ok(server.start(module))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cors_layer_empty_origins() {
        let config = CorsConfig::none();
        let _layer = build_cors_layer(&config);
    }

    #[test]
    fn cors_layer_specific_origins() {
        let config = CorsConfig {
            allowed_origins: vec!["http://localhost:3000".to_string()],
            allowed_methods: vec!["GET".to_string(), "POST".to_string()],
            allowed_headers: vec!["Content-Type".to_string()],
            max_age: 3600,
        };
        let _layer = build_cors_layer(&config);
    }

    #[test]
    fn cors_layer_wildcard() {
        let config = CorsConfig::permissive();
        let _layer = build_cors_layer(&config);
    }
}
