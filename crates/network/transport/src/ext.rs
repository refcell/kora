//! Extension trait for NetworkConfig to build transport.

use std::net::SocketAddr;

use commonware_cryptography::ed25519;
use commonware_p2p::Ingress;
use commonware_runtime::{Clock, Metrics, Network as RNetwork, Resolver, Spawner};
use kora_config::NetworkConfig;
use rand_core::CryptoRngCore;

use crate::{
    NetworkTransport, TransportConfig, TransportError, TransportParsing,
    config::{DEFAULT_MAX_MESSAGE_SIZE, DEFAULT_NAMESPACE},
};

/// Extension trait for building transport from network configuration.
pub trait NetworkConfigExt {
    /// Build a local development transport.
    ///
    /// Uses faster discovery and more lenient settings for local testing.
    fn build_local_transport<E>(
        &self,
        crypto: ed25519::PrivateKey,
        context: E,
    ) -> Result<NetworkTransport<ed25519::PublicKey, E>, TransportError>
    where
        E: Spawner + Clock + CryptoRngCore + RNetwork + Resolver + Metrics;

    /// Build a production transport.
    ///
    /// Uses conservative settings suitable for production deployments.
    fn build_transport<E>(
        &self,
        crypto: ed25519::PrivateKey,
        context: E,
    ) -> Result<NetworkTransport<ed25519::PublicKey, E>, TransportError>
    where
        E: Spawner + Clock + CryptoRngCore + RNetwork + Resolver + Metrics;
}

impl NetworkConfigExt for NetworkConfig {
    fn build_local_transport<E>(
        &self,
        crypto: ed25519::PrivateKey,
        context: E,
    ) -> Result<NetworkTransport<ed25519::PublicKey, E>, TransportError>
    where
        E: Spawner + Clock + CryptoRngCore + RNetwork + Resolver + Metrics,
    {
        let (listen_addr, dialable, bootstrappers) = parse_network_config(self)?;

        let transport_config = TransportConfig::local(
            crypto,
            DEFAULT_NAMESPACE,
            listen_addr,
            dialable,
            bootstrappers,
            DEFAULT_MAX_MESSAGE_SIZE,
        )
        .with_allow_private_ips(true);

        Ok(transport_config.build(context))
    }

    fn build_transport<E>(
        &self,
        crypto: ed25519::PrivateKey,
        context: E,
    ) -> Result<NetworkTransport<ed25519::PublicKey, E>, TransportError>
    where
        E: Spawner + Clock + CryptoRngCore + RNetwork + Resolver + Metrics,
    {
        let (listen_addr, dialable, bootstrappers) = parse_network_config(self)?;

        let transport_config = TransportConfig::recommended(
            crypto,
            DEFAULT_NAMESPACE,
            listen_addr,
            dialable,
            bootstrappers,
            DEFAULT_MAX_MESSAGE_SIZE,
        );

        Ok(transport_config.build(context))
    }
}

/// Bootstrapper list type alias.
type Bootstrappers = Vec<(ed25519::PublicKey, Ingress)>;

/// Parse network config into transport construction parameters.
fn parse_network_config(
    config: &NetworkConfig,
) -> Result<(SocketAddr, Ingress, Bootstrappers), TransportError> {
    let listen_addr: SocketAddr = config
        .listen_addr
        .parse()
        .map_err(|_| TransportError::InvalidListenAddr(config.listen_addr.clone()))?;

    // For now, use listen_addr as dialable address
    // TODO: Add dialable_addr field to NetworkConfig for NAT traversal
    let dialable = Ingress::Socket(listen_addr);

    let bootstrappers = TransportParsing::parse_bootstrappers(&config.bootstrap_peers)?;

    Ok((listen_addr, dialable, bootstrappers))
}
