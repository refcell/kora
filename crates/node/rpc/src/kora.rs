//! Kora-specific JSON-RPC API implementation.

use std::sync::Arc;

use jsonrpsee::{core::RpcResult, proc_macros::rpc};

use crate::state::{NodeState, NodeStatus};

/// Kora-specific JSON-RPC API trait.
///
/// Provides methods specific to Kora node operations.
#[rpc(server, namespace = "kora")]
pub trait KoraApi {
    /// Returns the current node status including consensus information.
    #[method(name = "nodeStatus")]
    async fn node_status(&self) -> RpcResult<NodeStatus>;
}

/// Implementation of the Kora RPC API.
#[derive(Debug)]
pub struct KoraApiImpl {
    state: Arc<NodeState>,
}

impl KoraApiImpl {
    /// Create a new Kora API implementation.
    pub fn new(state: Arc<NodeState>) -> Self {
        Self { state }
    }
}

#[jsonrpsee::core::async_trait]
impl KoraApiServer for KoraApiImpl {
    async fn node_status(&self) -> RpcResult<NodeStatus> {
        Ok(self.state.status())
    }
}
