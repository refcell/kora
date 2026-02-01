//! Node runner trait for delegating node wiring.
//!
//! The [`NodeRunner`] trait defines how a node is wired and started.
//! [`KoraNodeService`] delegates the actual node wiring to implementations
//! of this trait, allowing different execution environments (REVM, etc.)
//! to provide their own wiring logic.

use std::sync::Arc;

use commonware_runtime::tokio;
use kora_config::NodeConfig;

/// Context provided to a node runner.
///
/// This struct is `#[non_exhaustive]` to allow adding fields in future
/// iterations without breaking existing implementations.
#[non_exhaustive]
pub struct NodeRunContext<T> {
    context: tokio::Context,
    config: Arc<NodeConfig>,
    transport: T,
}

impl<T> std::fmt::Debug for NodeRunContext<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeRunContext").finish_non_exhaustive()
    }
}

impl<T> NodeRunContext<T> {
    /// Create a new run context.
    pub const fn new(context: tokio::Context, config: Arc<NodeConfig>, transport: T) -> Self {
        Self { context, config, transport }
    }

    /// Get a reference to the runtime context.
    pub const fn context(&self) -> &tokio::Context {
        &self.context
    }

    /// Get a clone of the runtime context.
    pub fn context_owned(&self) -> tokio::Context {
        self.context.clone()
    }

    /// Get the node configuration.
    pub const fn config(&self) -> &Arc<NodeConfig> {
        &self.config
    }

    /// Get a reference to the transport.
    pub const fn transport(&self) -> &T {
        &self.transport
    }

    /// Get a mutable reference to the transport.
    pub const fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Consume the context and return its parts.
    pub fn into_parts(self) -> (tokio::Context, Arc<NodeConfig>, T) {
        (self.context, self.config, self.transport)
    }
}

/// Trait for running a node with provided context and transport.
///
/// Implementations handle the actual node wiring: creating ledger state,
/// setting up consensus, starting the simplex engine, etc.
///
/// The service delegates to this trait after building the transport,
/// allowing different execution environments to provide their own logic.
///
/// # Type Parameters
///
/// - `Transport`: The transport type provided by the service
/// - `Handle`: What the runner returns for interacting with the running node
/// - `Error`: Error type for run failures
///
/// # Example
///
/// ```ignore
/// struct MyRunner { /* chain-specific config */ }
///
/// impl NodeRunner for MyRunner {
///     type Transport = simulated::Control<PublicKey, SimContext>;
///     type Handle = NodeHandle;
///     type Error = anyhow::Error;
///
///     async fn run(&self, ctx: NodeRunContext<Self::Transport>) -> Result<Self::Handle, Self::Error> {
///         let (context, config, transport) = ctx.into_parts();
///         // Wire up ledger, marshal, simplex, etc.
///         Ok(handle)
///     }
/// }
/// ```
pub trait NodeRunner: Send + Sync + 'static {
    /// The transport type this runner expects.
    type Transport: Send + Sync + 'static;

    /// Handle returned for interacting with the running node.
    type Handle: Send + 'static;

    /// Error type for run failures.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Run the node with the provided context.
    ///
    /// This method should:
    /// 1. Extract transport and context from `ctx`
    /// 2. Initialize ledger/state
    /// 3. Set up consensus components
    /// 4. Start background tasks
    /// 5. Return a handle for external interaction
    fn run(
        &self,
        ctx: NodeRunContext<Self::Transport>,
    ) -> impl std::future::Future<Output = Result<Self::Handle, Self::Error>> + Send;
}
