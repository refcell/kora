//! Transport provider trait for abstracting transport construction.

use commonware_cryptography::PublicKey;
use commonware_runtime::Clock;

use crate::bundle::TransportBundle;

/// Trait for building transport channel bundles.
///
/// This abstraction allows the same node code to work with both production
/// (authenticated discovery) and simulation (simulated P2P) transports.
///
/// The `Control` associated type allows simulation providers to expose
/// additional control capabilities (e.g., adding/removing network links)
/// without polluting production code.
pub trait TransportProvider<P: PublicKey, E: Clock> {
    /// Control handle type returned alongside the bundle.
    ///
    /// Production: `()` (no control needed)
    /// Simulation: Rich handle for network manipulation
    type Control: Send + 'static;

    /// Error type for transport construction failures.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Build the transport and return the channel bundle with control handle.
    #[allow(clippy::type_complexity)]
    fn build(self, context: E) -> Result<(TransportBundle<P, E>, Self::Control), Self::Error>;
}
