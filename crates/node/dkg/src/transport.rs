//! Production DKG transport using commonware-p2p authenticated discovery.
//!
//! This module provides authenticated, encrypted channels for DKG ceremony messages
//! using the commonware-p2p authenticated discovery network.

use std::num::NonZeroU32;

use bytes::Bytes;
use commonware_cryptography::ed25519;
use commonware_p2p::{
    Ingress, Manager, Receiver as ReceiverTrait, Recipients, Sender as SenderTrait,
    authenticated::discovery,
};
use commonware_runtime::{Clock, Handle, Metrics, Network, Quota, Resolver, Spawner};
use commonware_utils::ordered::Set;
use rand_core::CryptoRngCore;

use crate::{DkgConfig, DkgError};

/// Channel ID for DKG ceremony messages.
pub const CHANNEL_DKG: u64 = 10;

/// Default namespace for DKG network messages.
pub const DKG_NAMESPACE: &[u8] = b"_KORA_DKG_CEREMONY";

/// Default maximum message size for DKG (256 KB).
pub const DEFAULT_MAX_MESSAGE_SIZE: u32 = 256 * 1024;

/// Default channel backlog size.
pub const DEFAULT_BACKLOG: usize = 256;

/// Default rate quota (100 messages per second - DKG is low-volume).
const fn default_quota() -> Quota {
    Quota::per_second(NonZeroU32::new(100).expect("100 is non-zero"))
}

/// Type alias for DKG channel sender.
pub type DkgSender<E> = discovery::Sender<ed25519::PublicKey, E>;

/// Type alias for DKG channel receiver.
pub type DkgReceiver = discovery::Receiver<ed25519::PublicKey>;

/// Production DKG transport using authenticated discovery.
///
/// Wraps the commonware-p2p authenticated discovery network with a single
/// channel dedicated to DKG ceremony messages.
#[allow(missing_debug_implementations)]
pub struct DkgTransport<E: Clock> {
    /// Oracle for peer management.
    pub oracle: discovery::Oracle<ed25519::PublicKey>,

    /// Network handle - drop this to shut down the network.
    pub handle: Handle<()>,

    /// DKG message channel sender.
    pub sender: DkgSender<E>,

    /// DKG message channel receiver.
    pub receiver: DkgReceiver,
}

/// Configuration for building a DKG transport.
#[derive(Clone)]
#[allow(missing_debug_implementations)]
pub struct DkgTransportConfig {
    /// Inner discovery config.
    inner: discovery::Config<ed25519::PrivateKey>,

    /// Channel backlog size.
    backlog: usize,

    /// Rate quota for the DKG channel.
    quota: Quota,
}

impl DkgTransportConfig {
    /// Create a DKG transport config from a DkgConfig.
    ///
    /// Uses local discovery settings for faster peer discovery during ceremonies.
    pub fn from_dkg_config(config: &DkgConfig) -> Result<Self, DkgError> {
        let bootstrappers = config
            .bootstrap_peers
            .iter()
            .map(|(pk, addr)| {
                let ingress = parse_ingress(addr)?;
                Ok((pk.clone(), ingress))
            })
            .collect::<Result<Vec<_>, DkgError>>()?;

        let dialable = Ingress::Socket(config.listen_addr);

        Ok(Self {
            inner: discovery::Config::local(
                config.identity_key.clone(),
                DKG_NAMESPACE,
                config.listen_addr,
                dialable,
                bootstrappers,
                DEFAULT_MAX_MESSAGE_SIZE,
            ),
            backlog: DEFAULT_BACKLOG,
            quota: default_quota(),
        })
    }

    /// Create a production DKG transport config.
    ///
    /// Uses recommended (slower but more reliable) discovery settings.
    pub fn recommended(
        crypto: ed25519::PrivateKey,
        listen: std::net::SocketAddr,
        dialable: Ingress,
        bootstrappers: Vec<(ed25519::PublicKey, Ingress)>,
    ) -> Self {
        Self {
            inner: discovery::Config::recommended(
                crypto,
                DKG_NAMESPACE,
                listen,
                dialable,
                bootstrappers,
                DEFAULT_MAX_MESSAGE_SIZE,
            ),
            backlog: DEFAULT_BACKLOG,
            quota: default_quota(),
        }
    }

    /// Create a local development DKG transport config.
    ///
    /// Uses faster discovery settings for local testing.
    pub fn local(
        crypto: ed25519::PrivateKey,
        listen: std::net::SocketAddr,
        dialable: Ingress,
        bootstrappers: Vec<(ed25519::PublicKey, Ingress)>,
    ) -> Self {
        Self {
            inner: discovery::Config::local(
                crypto,
                DKG_NAMESPACE,
                listen,
                dialable,
                bootstrappers,
                DEFAULT_MAX_MESSAGE_SIZE,
            ),
            backlog: DEFAULT_BACKLOG,
            quota: default_quota(),
        }
    }

    /// Set the channel backlog size.
    pub const fn with_backlog(mut self, backlog: usize) -> Self {
        self.backlog = backlog;
        self
    }

    /// Set a custom rate quota.
    pub const fn with_quota(mut self, quota: Quota) -> Self {
        self.quota = quota;
        self
    }

    /// Allow private IP addresses for connections.
    pub const fn with_allow_private_ips(mut self, allow: bool) -> Self {
        self.inner.allow_private_ips = allow;
        self
    }

    /// Allow DNS-based peer addresses.
    pub const fn with_allow_dns(mut self, allow: bool) -> Self {
        self.inner.allow_dns = allow;
        self
    }

    /// Build the DKG transport.
    ///
    /// Creates the authenticated discovery network with a single DKG channel
    /// and starts the network.
    pub fn build<E>(self, context: E) -> DkgTransport<E>
    where
        E: Spawner + Clock + CryptoRngCore + Network + Resolver + Metrics,
    {
        let (mut network, oracle) =
            discovery::Network::new(context.with_label("dkg-network"), self.inner);

        let (sender, receiver) = network.register(CHANNEL_DKG, self.quota, self.backlog);

        let handle = network.start();

        tracing::info!("DKG transport started with authenticated discovery");

        DkgTransport { oracle, handle, sender, receiver }
    }
}

impl<E: Clock> DkgTransport<E> {
    /// Update the set of authorized participants.
    ///
    /// This should be called with the DKG ceremony participants before starting.
    pub async fn set_participants(&mut self, participants: Set<ed25519::PublicKey>) {
        self.oracle.update(0, participants).await;
    }

    /// Send a message to a specific peer.
    pub async fn send_to(&mut self, to: &ed25519::PublicKey, msg: Bytes) -> Result<(), DkgError>
    where
        E: Spawner + Clock + CryptoRngCore + Network,
    {
        self.sender
            .send(Recipients::One(to.clone()), msg, false)
            .await
            .map(|_| ())
            .map_err(|e| DkgError::Network(format!("Failed to send to peer: {}", e)))
    }

    /// Broadcast a message to all connected peers.
    pub async fn broadcast(&mut self, msg: Bytes) -> Result<(), DkgError>
    where
        E: Spawner + Clock + CryptoRngCore + Network,
    {
        self.sender
            .send(Recipients::All, msg, false)
            .await
            .map(|_| ())
            .map_err(|e| DkgError::Network(format!("Failed to broadcast: {}", e)))
    }

    /// Receive the next message.
    ///
    /// Returns the sender's public key and the message bytes.
    pub async fn recv(&mut self) -> Option<(ed25519::PublicKey, Bytes)> {
        self.receiver.recv().await.ok()
    }
}

/// Parse an address string into an Ingress.
fn parse_ingress(addr_str: &str) -> Result<Ingress, DkgError> {
    if let Ok(socket) = addr_str.parse::<std::net::SocketAddr>() {
        return Ok(Ingress::Socket(socket));
    }

    let (host, port_str) = addr_str
        .rsplit_once(':')
        .ok_or_else(|| DkgError::Network(format!("Invalid address format: {}", addr_str)))?;

    let port: u16 =
        port_str.parse().map_err(|_| DkgError::Network(format!("Invalid port: {}", port_str)))?;

    let hostname = commonware_utils::Hostname::new(host)
        .map_err(|_| DkgError::Network(format!("Invalid hostname: {}", host)))?;

    Ok(Ingress::Dns { host: hostname, port })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ingress_socket() {
        let result = parse_ingress("127.0.0.1:8080").unwrap();
        assert!(matches!(result, Ingress::Socket(_)));
    }

    #[test]
    fn test_parse_ingress_dns() {
        let result = parse_ingress("node.example.com:8080").unwrap();
        assert!(matches!(result, Ingress::Dns { .. }));
    }

    #[test]
    fn test_parse_ingress_invalid() {
        assert!(parse_ingress("invalid").is_err());
    }
}
