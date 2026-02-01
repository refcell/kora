//! Transport configuration.

use std::net::SocketAddr;

use commonware_codec::{FixedSize, ReadExt};
use commonware_cryptography::ed25519;
use commonware_p2p::{Ingress, authenticated::discovery};

use crate::error::TransportError;

/// Default maximum message size (1 MB).
pub const DEFAULT_MAX_MESSAGE_SIZE: u32 = 1024 * 1024;

/// Default channel backlog size.
pub const DEFAULT_BACKLOG: usize = 256;

/// Default namespace for kora network messages.
pub const DEFAULT_NAMESPACE: &[u8] = b"_COMMONWARE_KORA_NETWORK";

/// Transport configuration for authenticated discovery network.
///
/// This wraps the commonware discovery config with kora-specific defaults
/// and provides builder methods for customization.
#[derive(Clone)]
#[allow(missing_debug_implementations)]
pub struct TransportConfig<C: commonware_cryptography::Signer> {
    /// Inner discovery config.
    pub(crate) inner: discovery::Config<C>,

    /// Channel backlog size.
    pub(crate) backlog: usize,
}

/// Parsing helpers for transport configuration.
#[derive(Debug)]
pub struct TransportParsing;

impl<C: commonware_cryptography::Signer> TransportConfig<C> {
    /// Create a recommended production configuration.
    ///
    /// Uses conservative settings suitable for production deployments.
    pub fn recommended(
        crypto: C,
        namespace: &[u8],
        listen: SocketAddr,
        dialable: Ingress,
        bootstrappers: Vec<(C::PublicKey, Ingress)>,
        max_message_size: u32,
    ) -> Self {
        Self {
            inner: discovery::Config::recommended(
                crypto,
                namespace,
                listen,
                dialable,
                bootstrappers,
                max_message_size,
            ),
            backlog: DEFAULT_BACKLOG,
        }
    }

    /// Create a local development configuration.
    ///
    /// Uses faster discovery and more lenient settings for local testing.
    pub fn local(
        crypto: C,
        namespace: &[u8],
        listen: SocketAddr,
        dialable: Ingress,
        bootstrappers: Vec<(C::PublicKey, Ingress)>,
        max_message_size: u32,
    ) -> Self {
        Self {
            inner: discovery::Config::local(
                crypto,
                namespace,
                listen,
                dialable,
                bootstrappers,
                max_message_size,
            ),
            backlog: DEFAULT_BACKLOG,
        }
    }

    /// Set the channel backlog size.
    #[must_use]
    pub const fn with_backlog(mut self, backlog: usize) -> Self {
        self.backlog = backlog;
        self
    }

    /// Allow private IP addresses for connections.
    #[must_use]
    pub const fn with_allow_private_ips(mut self, allow: bool) -> Self {
        self.inner.allow_private_ips = allow;
        self
    }

    /// Allow DNS-based peer addresses.
    #[must_use]
    pub const fn with_allow_dns(mut self, allow: bool) -> Self {
        self.inner.allow_dns = allow;
        self
    }
}

impl TransportParsing {
    /// Parse a dialable address string into an [`Ingress`].
    ///
    /// Supports both IP:port and hostname:port formats.
    pub fn parse_ingress(addr_str: &str) -> Result<Ingress, TransportError> {
        if let Ok(socket) = addr_str.parse::<SocketAddr>() {
            return Ok(Ingress::Socket(socket));
        }

        let (host, port_str) = addr_str
            .rsplit_once(':')
            .ok_or_else(|| TransportError::InvalidDialableAddr(addr_str.to_string()))?;

        let port: u16 =
            port_str.parse().map_err(|_| TransportError::InvalidPort(port_str.to_string()))?;

        let hostname = commonware_utils::Hostname::new(host)
            .map_err(|_| TransportError::InvalidHostname(host.to_string()))?;

        Ok(Ingress::Dns { host: hostname, port })
    }

    /// Parse a listen address string into a [`SocketAddr`].
    pub fn parse_listen_addr(addr_str: &str) -> Result<SocketAddr, TransportError> {
        addr_str.parse().map_err(|_| TransportError::InvalidListenAddr(addr_str.to_string()))
    }

    /// Parse bootstrap peer strings into (PublicKey, Ingress) tuples.
    ///
    /// Expected format: `PUBLIC_KEY_HEX@HOST:PORT`
    pub fn parse_bootstrappers(
        bootstrap_peers: &[String],
    ) -> Result<Vec<(ed25519::PublicKey, Ingress)>, TransportError> {
        bootstrap_peers
            .iter()
            .map(|peer_str| {
                let (pk_hex, addr) = peer_str
                    .split_once('@')
                    .ok_or_else(|| TransportError::InvalidBootstrapPeer(peer_str.clone()))?;

                let pk_hex = pk_hex.strip_prefix("0x").unwrap_or(pk_hex);
                let pk_bytes = hex::decode(pk_hex)
                    .map_err(|_| TransportError::InvalidPublicKeyHex(pk_hex.to_string()))?;

                if pk_bytes.len() != ed25519::PublicKey::SIZE {
                    return Err(TransportError::InvalidPublicKeyLength(pk_bytes.len()));
                }

                let mut buf = pk_bytes.as_slice();
                let public_key = ed25519::PublicKey::read(&mut buf)
                    .map_err(|_| TransportError::InvalidPublicKey)?;

                let ingress = Self::parse_ingress(addr)?;

                Ok((public_key, ingress))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ingress_ipv4() {
        let result = TransportParsing::parse_ingress("192.168.1.1:30303").unwrap();
        assert!(matches!(result, Ingress::Socket(_)));
    }

    #[test]
    fn parse_ingress_ipv6() {
        let result = TransportParsing::parse_ingress("[::1]:8080").unwrap();
        assert!(matches!(result, Ingress::Socket(_)));
    }

    #[test]
    fn parse_ingress_dns() {
        let result = TransportParsing::parse_ingress("node.example.com:30303").unwrap();
        assert!(matches!(result, Ingress::Dns { .. }));
    }

    #[test]
    fn parse_ingress_missing_port() {
        let result = TransportParsing::parse_ingress("192.168.1.1");
        assert!(matches!(result, Err(TransportError::InvalidDialableAddr(_))));
    }

    #[test]
    fn parse_listen_addr_valid() {
        let result = TransportParsing::parse_listen_addr("0.0.0.0:30303").unwrap();
        assert_eq!(result.port(), 30303);
    }

    #[test]
    fn parse_listen_addr_invalid() {
        let result = TransportParsing::parse_listen_addr("not-an-address");
        assert!(matches!(result, Err(TransportError::InvalidListenAddr(_))));
    }

    #[test]
    fn parse_bootstrappers_valid() {
        let pk = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
        let peers = vec![format!("{}@192.168.1.1:30303", pk)];
        let result = TransportParsing::parse_bootstrappers(&peers).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn parse_bootstrappers_empty() {
        let peers: Vec<String> = vec![];
        let result = TransportParsing::parse_bootstrappers(&peers).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_bootstrappers_invalid_format() {
        let peers = vec!["no-at-symbol".to_string()];
        let result = TransportParsing::parse_bootstrappers(&peers);
        assert!(matches!(result, Err(TransportError::InvalidBootstrapPeer(_))));
    }

    #[test]
    fn constants_values() {
        assert_eq!(DEFAULT_MAX_MESSAGE_SIZE, 1024 * 1024);
        assert_eq!(DEFAULT_BACKLOG, 256);
        assert_eq!(DEFAULT_NAMESPACE, b"_COMMONWARE_KORA_NETWORK");
    }
}
