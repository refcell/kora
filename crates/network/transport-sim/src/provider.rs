//! Simulated transport provider implementation.

use std::time::Duration;

use commonware_cryptography::PublicKey;
use commonware_p2p::{Manager as _, simulated};
use commonware_runtime::Quota;

use kora_transport::{CHANNEL_BACKFILL, CHANNEL_BLOCKS, CHANNEL_CERTS, CHANNEL_RESOLVER, CHANNEL_VOTES};

use crate::{SimContext, SimTransportError};
use crate::channels::{SimMarshalChannels, SimSimplexChannels};

/// Configuration for simulated network links.
#[derive(Debug, Clone)]
pub struct SimLinkConfig {
    /// Link latency.
    pub latency: Duration,
    /// Latency jitter.
    pub jitter: Duration,
    /// Success rate (0.0 to 1.0).
    pub success_rate: f64,
}

impl Default for SimLinkConfig {
    fn default() -> Self {
        Self { latency: Duration::from_millis(5), jitter: Duration::ZERO, success_rate: 1.0 }
    }
}

impl From<SimLinkConfig> for simulated::Link {
    fn from(cfg: SimLinkConfig) -> Self {
        Self { latency: cfg.latency, jitter: cfg.jitter, success_rate: cfg.success_rate }
    }
}

/// Control handle for simulated network manipulation.
#[allow(missing_debug_implementations)]
pub struct SimControl<P: PublicKey> {
    /// Simulated network oracle.
    pub oracle: simulated::Oracle<P, SimContext>,
}

impl<P: PublicKey> SimControl<P> {
    /// Creates a new control handle wrapping an oracle.
    pub const fn new(oracle: simulated::Oracle<P, SimContext>) -> Self {
        Self { oracle }
    }

    /// Adds a network link between two peers.
    pub async fn add_link(
        &mut self,
        from: P,
        to: P,
        link: simulated::Link,
    ) -> Result<(), simulated::Error> {
        self.oracle.add_link(from, to, link).await
    }

    /// Updates the validator set for an epoch.
    pub async fn update_validators(
        &self,
        epoch: u64,
        validators: commonware_utils::ordered::Set<P>,
    ) {
        self.manager().update(epoch, validators).await;
    }

    /// Returns a peer control handle for channel registration.
    pub fn peer_control(&self, peer: P) -> simulated::Control<P, SimContext> {
        self.oracle.control(peer)
    }

    /// Returns the network manager.
    pub fn manager(&self) -> simulated::Manager<P, SimContext> {
        self.oracle.manager()
    }
}

/// Registered channel bundle for a simulated node.
#[allow(missing_debug_implementations)]
pub struct SimChannels<P: PublicKey> {
    /// Simplex consensus channels.
    pub simplex: SimSimplexChannels<P>,
    /// Marshal block dissemination channels.
    pub marshal: SimMarshalChannels<P>,
}

/// Registers all required channels for a node.
pub async fn register_node_channels<P: PublicKey>(
    control: &mut simulated::Control<P, SimContext>,
    quota: Quota,
) -> Result<SimChannels<P>, SimTransportError> {
    let votes = control
        .register(CHANNEL_VOTES, quota)
        .await
        .map_err(|e| SimTransportError::ChannelRegistration(format!("votes: {e}")))?;
    let certs = control
        .register(CHANNEL_CERTS, quota)
        .await
        .map_err(|e| SimTransportError::ChannelRegistration(format!("certs: {e}")))?;
    let resolver = control
        .register(CHANNEL_RESOLVER, quota)
        .await
        .map_err(|e| SimTransportError::ChannelRegistration(format!("resolver: {e}")))?;
    let blocks = control
        .register(CHANNEL_BLOCKS, quota)
        .await
        .map_err(|e| SimTransportError::ChannelRegistration(format!("blocks: {e}")))?;
    let backfill = control
        .register(CHANNEL_BACKFILL, quota)
        .await
        .map_err(|e| SimTransportError::ChannelRegistration(format!("backfill: {e}")))?;

    Ok(SimChannels {
        simplex: SimSimplexChannels { votes, certs, resolver },
        marshal: SimMarshalChannels { blocks, backfill },
    })
}

/// Creates a new simulated network.
pub fn create_sim_network<P: PublicKey>(
    context: SimContext,
    max_size: u32,
    disconnect_on_block: bool,
) -> SimControl<P> {
    let config =
        simulated::Config { max_size, disconnect_on_block, tracked_peer_sets: None };

    let (network, oracle) = simulated::Network::new(context, config);
    network.start();

    SimControl::new(oracle)
}
