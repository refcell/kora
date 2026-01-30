//! Networking layer for DKG ceremony using TCP sockets.
//!
//! This module provides simple TCP-based networking for the DKG ceremony.
//! For production, this should be replaced with proper authenticated channels.

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sync::{Arc, Mutex},
    time::Duration,
};

use commonware_codec::Write as CodecWrite;
use commonware_cryptography::ed25519;
use tracing::{debug, error, info, warn};

use crate::{DkgConfig, DkgError, protocol::ProtocolMessage};

/// Simple message envelope with sender info.
#[derive(Debug, Clone)]
pub(crate) struct Envelope {
    pub from: ed25519::PublicKey,
    pub payload: Vec<u8>,
}

/// Network handle for sending and receiving DKG messages.
pub struct DkgNetwork {
    config: DkgConfig,
    listener: TcpListener,
    peer_addrs: HashMap<ed25519::PublicKey, String>,
    incoming: Arc<Mutex<Vec<Envelope>>>,
}

impl DkgNetwork {
    /// Create a new DKG network.
    pub fn new(config: DkgConfig) -> Result<Self, DkgError> {
        let listener = TcpListener::bind(config.listen_addr)
            .map_err(|e| DkgError::Network(format!("Failed to bind: {}", e)))?;

        listener
            .set_nonblocking(true)
            .map_err(|e| DkgError::Network(format!("Failed to set nonblocking: {}", e)))?;

        // Build peer address map
        let mut peer_addrs = HashMap::new();
        for (pk, addr) in &config.bootstrap_peers {
            peer_addrs.insert(pk.clone(), addr.clone());
        }

        info!(
            addr = %config.listen_addr,
            peers = peer_addrs.len(),
            "DKG network initialized"
        );

        Ok(Self { config, listener, peer_addrs, incoming: Arc::new(Mutex::new(Vec::new())) })
    }

    /// Send a message to a specific peer.
    pub fn send_to(&self, to: &ed25519::PublicKey, msg: &ProtocolMessage) -> Result<(), DkgError> {
        let addr = self
            .peer_addrs
            .get(to)
            .ok_or_else(|| DkgError::Network(format!("Unknown peer: {:?}", to)))?;

        let payload = msg.to_bytes();
        let mut envelope = Vec::new();

        // Write our public key
        self.config.my_public_key().write(&mut envelope);
        // Write payload length
        (payload.len() as u32).to_le_bytes().iter().for_each(|b| envelope.push(*b));
        // Write payload
        envelope.extend_from_slice(&payload);

        // Resolve the address (supports both IP:port and hostname:port)
        let socket_addr = addr
            .to_socket_addrs()
            .map_err(|e| DkgError::Network(format!("Failed to resolve {}: {}", addr, e)))?
            .next()
            .ok_or_else(|| DkgError::Network(format!("No addresses found for {}", addr)))?;

        match TcpStream::connect_timeout(&socket_addr, Duration::from_secs(5)) {
            Ok(mut stream) => {
                stream
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .map_err(|e| DkgError::Network(format!("Set timeout: {}", e)))?;
                stream
                    .write_all(&envelope)
                    .map_err(|e| DkgError::Network(format!("Write: {}", e)))?;
                debug!(?to, len = payload.len(), "Sent message");
                Ok(())
            }
            Err(e) => {
                warn!(?to, %addr, %e, "Failed to connect to peer");
                Err(DkgError::Network(format!("Connect: {}", e)))
            }
        }
    }

    /// Broadcast a message to all peers.
    pub fn broadcast(&self, msg: &ProtocolMessage) -> Result<(), DkgError> {
        let my_pk = self.config.my_public_key();
        for pk in self.config.participants.iter() {
            if pk != &my_pk
                && let Err(e) = self.send_to(pk, msg)
            {
                warn!(?pk, ?e, "Failed to send to peer");
            }
        }
        Ok(())
    }

    /// Poll for incoming messages (non-blocking).
    pub(crate) fn poll_incoming(&self) -> Vec<Envelope> {
        let mut result = Vec::new();

        // Accept new connections
        loop {
            match self.listener.accept() {
                Ok((mut stream, addr)) => {
                    debug!(%addr, "Accepted connection");

                    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

                    // Read public key (32 bytes for ed25519)
                    let mut pk_bytes = [0u8; 32];
                    if stream.read_exact(&mut pk_bytes).is_err() {
                        warn!(%addr, "Failed to read sender public key");
                        continue;
                    }

                    let from = match commonware_codec::ReadExt::read(&mut pk_bytes.as_slice()) {
                        Ok(pk) => pk,
                        Err(e) => {
                            warn!(%addr, ?e, "Failed to decode public key");
                            continue;
                        }
                    };

                    // Read payload length
                    let mut len_bytes = [0u8; 4];
                    if stream.read_exact(&mut len_bytes).is_err() {
                        warn!(%addr, "Failed to read length");
                        continue;
                    }
                    let len = u32::from_le_bytes(len_bytes) as usize;

                    if len > 1024 * 1024 {
                        warn!(%addr, len, "Message too large");
                        continue;
                    }

                    // Read payload
                    let mut payload = vec![0u8; len];
                    if stream.read_exact(&mut payload).is_err() {
                        warn!(%addr, "Failed to read payload");
                        continue;
                    }

                    result.push(Envelope { from, payload });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(e) => {
                    error!(%e, "Accept error");
                    break;
                }
            }
        }

        result
    }

    /// Get the maximum polynomial degree for message parsing.
    pub const fn max_degree(&self) -> u32 {
        // degree = quorum - 1, quorum = n - f, f = (n-1)/3
        // For n=4: f=1, quorum=3, degree=2
        let n = self.config.participants.len() as u32;
        let f = (n - 1) / 3;
        // Use quorum as max, actual degree is quorum-1 but we need margin
        n - f
    }
}
