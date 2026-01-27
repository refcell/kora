//! Kora node service implementation.

use kora_config::NodeConfig;

/// The main kora node service.
#[derive(Debug)]
pub struct KoraNodeService {
    config: NodeConfig,
}

impl KoraNodeService {
    /// Create a new [`KoraNodeService`].
    pub const fn new(config: NodeConfig) -> Self {
        Self { config }
    }

    /// Run the kora node service.
    pub fn run(self) -> eyre::Result<()> {
        let executor = tokio::Runner::default();
        executor.start(|context| async move { self.run_with_context(context).await })
    }

    /// Runs the kora node service with context.
    pub async fn run_with_context(self) -> eyre::Result<()> {
        // Load or create validator key
        let validator: PublicKey = self.config.validator_public_key()?;

        // TODO: construct the real production network here

        // TODO: construct the application

        // TODO: Start simplex.

        Ok(())
    }

    //     use std::num::NonZeroU32;
    //
    //     use commonware_p2p::simulated::{self, Network};
    //     use commonware_runtime::{Metrics as _, Quota, Runner, tokio};
    //
    //     use crate::stubs::{StubAutomaton, StubRelay, StubReporter};
    //
    //     // Type aliases
    //     type PublicKey = commonware_cryptography::ed25519::PublicKey;
    //     type Scheme = commonware_consensus::simplex::scheme::bls12381_threshold::Scheme<
    //         PublicKey,
    //         commonware_cryptography::bls12381::primitives::variant::MinSig,
    //     >;
    //
    //     // Use tokio runtime
    //     let runner = tokio::Runner::default();
    //     runner.start(|context| async move {
    //         // Create simulated network for development
    //         let (network, oracle) = Network::new(
    //             context.with_label("network"),
    //             simulated::Config {
    //                 max_size: 1024 * 1024,
    //                 disconnect_on_block: true,
    //                 tracked_peer_sets: None,
    //             },
    //         );
    //         network.start();
    //
    //
    //
    //         // Register network channels
    //         let quota = Quota::per_second(NonZeroU32::MAX);
    //         let control = oracle.control(validator.clone());
    //
    //         // Channel IDs for different message types
    //         const VOTES_CHANNEL: u64 = 0;
    //         const CERTS_CHANNEL: u64 = 1;
    //         const RESOLVER_CHANNEL: u64 = 2;
    //
    //         let votes = control.register(VOTES_CHANNEL, quota).await.expect("register votes");
    //         let certs = control.register(CERTS_CHANNEL, quota).await.expect("register certs");
    //         let resolver =
    //             control.register(RESOLVER_CHANNEL, quota).await.expect("register resolver");
    //
    //         tracing::info!(
    //             validator = ?validator,
    //             "Network channels registered"
    //         );
    //
    //         // Create stub components
    //         let automaton = StubAutomaton;
    //         let relay = StubRelay;
    //         let reporter: StubReporter<Scheme> = StubReporter::default();
    //
    //         tracing::info!("Simplex components created (stubs)");
    //
    //         // Note: Cannot start engine without a real scheme
    //         // The scheme requires the mocks feature which is not available
    //         // on the published crate. When using commonware from git or
    //         // with mocks feature enabled, the engine can be started.
    //         //
    //         // For now, keep the components alive to show the wiring works.
    //         let _ = (automaton, relay, reporter, votes, certs, resolver);
    //
    //         tracing::info!(
    //             chain_id = self.config.chain_id,
    //             "Kora node service initialized (network + stubs ready)"
    //         );
    //
    //         // Wait indefinitely
    //         futures::future::pending::<()>().await;
    //     });
    //
    //     Ok(())
    // }
}
