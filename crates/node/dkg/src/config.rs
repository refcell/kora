use std::{path::PathBuf, time::Duration};

use commonware_cryptography::ed25519;

/// Configuration for a Distributed Key Generation (DKG) ceremony.
#[derive(Debug, Clone)]
pub struct DkgConfig {
    /// The validator's private identity key used for signing and authentication.
    pub identity_key: ed25519::PrivateKey,
    /// This validator's index in the participant set.
    pub validator_index: usize,
    /// Public keys of all validators participating in the DKG ceremony.
    pub participants: Vec<ed25519::PublicKey>,
    /// Minimum number of participants required to reconstruct the secret (t-of-n).
    pub threshold: u32,
    /// Chain identifier for domain separation.
    pub chain_id: u64,
    /// Directory for persisting DKG state and key shares.
    pub data_dir: PathBuf,
    /// Socket address to listen on for P2P communication.
    pub listen_addr: std::net::SocketAddr,
    /// Initial peers to connect to, as (public_key, address) pairs.
    pub bootstrap_peers: Vec<(ed25519::PublicKey, String)>,
    /// Timeout duration for DKG protocol rounds.
    pub timeout: Duration,
}

impl DkgConfig {
    /// Returns the total number of participants (n).
    pub const fn n(&self) -> usize {
        self.participants.len()
    }

    /// Returns the threshold value (t).
    pub const fn t(&self) -> u32 {
        self.threshold
    }

    /// Returns this validator's public key derived from the identity key.
    pub fn my_public_key(&self) -> ed25519::PublicKey {
        use commonware_cryptography::Signer;
        self.identity_key.public_key()
    }
}
