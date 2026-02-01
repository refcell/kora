//! Production network transport provider implementation.

use std::fmt;

use commonware_cryptography::Signer;
use commonware_p2p::authenticated::discovery;
use commonware_runtime::{Clock, Metrics, Network as RNetwork, Quota, Resolver, Spawner};
use rand_core::CryptoRngCore;

use crate::{
    TransportBundle, TransportConfig, TransportError, TransportProvider,
    channels::{
        CHANNEL_BACKFILL, CHANNEL_BLOCKS, CHANNEL_CERTS, CHANNEL_RESOLVER, CHANNEL_VOTES,
        MarshalChannels, SimplexChannels,
    },
};

/// Production transport provider using authenticated discovery.
///
/// Wraps a [`TransportConfig`] and builds the real P2P network on demand.
pub struct NetworkTransportProvider<C: Signer> {
    config: TransportConfig<C>,
    quota: Quota,
}

impl<C: Signer> fmt::Debug for NetworkTransportProvider<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetworkTransportProvider")
            .field("quota", &self.quota)
            .finish_non_exhaustive()
    }
}

impl<C: Signer> NetworkTransportProvider<C> {
    /// Create a new provider from transport configuration.
    pub const fn new(config: TransportConfig<C>, quota: Quota) -> Self {
        Self { config, quota }
    }
}

use commonware_cryptography::PublicKey;

/// Oracle handle returned by production transport.
///
/// Allows the caller to manage the validator set and block misbehaving peers.
pub struct NetworkControl<P: PublicKey> {
    /// Oracle for peer management and Byzantine blocking.
    pub oracle: discovery::Oracle<P>,
}

impl<P: PublicKey> fmt::Debug for NetworkControl<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetworkControl").finish_non_exhaustive()
    }
}

impl<C, E> TransportProvider<C::PublicKey, E> for NetworkTransportProvider<C>
where
    C: Signer,
    E: Spawner + Clock + CryptoRngCore + RNetwork + Resolver + Metrics,
{
    type Control = NetworkControl<C::PublicKey>;
    type Error = TransportError;

    fn build(
        self,
        context: E,
    ) -> Result<(TransportBundle<C::PublicKey, E>, Self::Control), Self::Error> {
        let backlog = self.config.backlog;

        let (mut network, oracle) =
            discovery::Network::new(context.with_label("network"), self.config.inner);

        let votes = network.register(CHANNEL_VOTES, self.quota, backlog);
        let certs = network.register(CHANNEL_CERTS, self.quota, backlog);
        let resolver = network.register(CHANNEL_RESOLVER, self.quota, backlog);
        let blocks = network.register(CHANNEL_BLOCKS, self.quota, backlog);
        let backfill = network.register(CHANNEL_BACKFILL, self.quota, backlog);

        let handle = network.start();

        tracing::info!("network transport started with 5 channels");

        let bundle = TransportBundle::new(
            SimplexChannels { votes, certs, resolver },
            MarshalChannels { blocks, backfill },
            handle,
        );

        let control = NetworkControl { oracle };

        Ok((bundle, control))
    }
}
