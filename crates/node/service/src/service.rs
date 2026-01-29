//! Kora node service implementation.

use std::sync::Arc;

use commonware_cryptography::Signer;
use commonware_p2p::Manager;
use commonware_runtime::{
    Runner,
    tokio::{self, Context},
};
use futures::future::try_join_all;
use kora_config::NodeConfig;
use kora_transport::NetworkConfigExt;

use crate::{NodeRunContext, NodeRunner, TransportProvider};

/// Generic kora node service that delegates to a runner.
///
/// This is the primary way to run a kora node with custom execution logic.
/// The service handles transport creation via the `TransportProvider`,
/// then delegates node wiring to the `NodeRunner`.
#[allow(missing_debug_implementations)]
pub struct KoraNodeService<R, T>
where
    R: NodeRunner<Transport = T::Transport>,
    T: TransportProvider,
{
    runner: R,
    transport_provider: T,
    config: NodeConfig,
}

impl<R, T> KoraNodeService<R, T>
where
    R: NodeRunner<Transport = T::Transport>,
    T: TransportProvider,
{
    /// Create a new generic node service.
    pub const fn new(runner: R, transport_provider: T, config: NodeConfig) -> Self {
        Self { runner, transport_provider, config }
    }

    /// Run the node service using the default tokio runtime.
    pub fn run(self) -> Result<R::Handle, eyre::Error>
    where
        R::Error: Into<eyre::Error>,
        T::Error: Into<eyre::Error>,
    {
        let executor = tokio::Runner::default();
        executor.start(|context| async move { self.run_with_context(context).await })
    }

    /// Run the node service with a provided context.
    pub async fn run_with_context(mut self, context: Context) -> Result<R::Handle, eyre::Error>
    where
        R::Error: Into<eyre::Error>,
        T::Error: Into<eyre::Error>,
    {
        let transport = self
            .transport_provider
            .build_transport(&context, &self.config)
            .await
            .map_err(Into::into)?;

        let run_ctx = NodeRunContext::new(context, Arc::new(self.config), transport);

        self.runner.run(run_ctx).await.map_err(Into::into)
    }
}

/// Legacy kora node service for production use.
///
/// This maintains backward compatibility with the existing production binary.
/// For new implementations, prefer [`KoraNodeService`] with custom runner/provider.
#[derive(Debug)]
pub struct LegacyNodeService {
    config: NodeConfig,
}

impl LegacyNodeService {
    /// Create a new legacy node service.
    pub const fn new(config: NodeConfig) -> Self {
        Self { config }
    }

    /// Run the legacy node service.
    pub fn run(self) -> eyre::Result<()> {
        let executor = tokio::Runner::default();
        executor.start(|context| async move { self.run_with_context(context).await })
    }

    /// Runs the legacy node service with context.
    pub async fn run_with_context(self, context: Context) -> eyre::Result<()> {
        let validator_key = self.config.validator_key()?;
        let validator = validator_key.public_key();
        tracing::info!(?validator, "loaded validator key");

        let mut transport = self
            .config
            .network
            .build_local_transport(validator_key, context.clone())
            .map_err(|e| eyre::eyre!("failed to build transport: {}", e))?;
        tracing::info!("network transport started");

        let validators = self.config.consensus.build_validator_set()?;
        if !validators.is_empty() {
            transport.oracle.update(0, validators.try_into().expect("valid set")).await;
            tracing::info!("registered validators with oracle");
        }

        tracing::info!(chain_id = self.config.chain_id, "kora node initialized");

        if let Err(e) = try_join_all(vec![transport.handle]).await {
            tracing::error!(?e, "service task failed");
            return Err(eyre::eyre!("service task failed: {:?}", e));
        }

        tracing::info!("kora node shutdown");
        Ok(())
    }
}
