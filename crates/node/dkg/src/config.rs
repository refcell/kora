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

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use commonware_cryptography::Signer;

    use super::*;

    /// Helper function to create a test DkgConfig with default values.
    fn test_config() -> DkgConfig {
        let identity_key = ed25519::PrivateKey::from_seed(42);
        let participants = vec![
            ed25519::PrivateKey::from_seed(42).public_key(),
            ed25519::PrivateKey::from_seed(43).public_key(),
            ed25519::PrivateKey::from_seed(44).public_key(),
            ed25519::PrivateKey::from_seed(45).public_key(),
        ];

        DkgConfig {
            identity_key,
            validator_index: 0,
            participants,
            threshold: 3,
            chain_id: 1337,
            data_dir: PathBuf::from("/tmp/dkg-test"),
            listen_addr: "127.0.0.1:8000".parse::<SocketAddr>().unwrap(),
            bootstrap_peers: vec![],
            timeout: Duration::from_secs(60),
        }
    }

    #[test]
    fn test_n_returns_participant_count() {
        let config = test_config();
        assert_eq!(config.n(), 4);
    }

    #[test]
    fn test_n_with_single_participant() {
        let identity_key = ed25519::PrivateKey::from_seed(42);
        let config = DkgConfig {
            identity_key,
            validator_index: 0,
            participants: vec![ed25519::PrivateKey::from_seed(42).public_key()],
            threshold: 1,
            chain_id: 1337,
            data_dir: PathBuf::from("/tmp/dkg-test"),
            listen_addr: "127.0.0.1:8000".parse::<SocketAddr>().unwrap(),
            bootstrap_peers: vec![],
            timeout: Duration::from_secs(60),
        };
        assert_eq!(config.n(), 1);
    }

    #[test]
    fn test_n_with_many_participants() {
        let identity_key = ed25519::PrivateKey::from_seed(42);
        let participants: Vec<_> =
            (0..100).map(|i| ed25519::PrivateKey::from_seed(i as u64).public_key()).collect();

        let config = DkgConfig {
            identity_key,
            validator_index: 0,
            participants,
            threshold: 67,
            chain_id: 1337,
            data_dir: PathBuf::from("/tmp/dkg-test"),
            listen_addr: "127.0.0.1:8000".parse::<SocketAddr>().unwrap(),
            bootstrap_peers: vec![],
            timeout: Duration::from_secs(60),
        };
        assert_eq!(config.n(), 100);
    }

    #[test]
    fn test_t_returns_threshold() {
        let config = test_config();
        assert_eq!(config.t(), 3);
    }

    #[test]
    fn test_t_with_threshold_one() {
        let identity_key = ed25519::PrivateKey::from_seed(42);
        let config = DkgConfig {
            identity_key,
            validator_index: 0,
            participants: vec![ed25519::PrivateKey::from_seed(42).public_key()],
            threshold: 1,
            chain_id: 1337,
            data_dir: PathBuf::from("/tmp/dkg-test"),
            listen_addr: "127.0.0.1:8000".parse::<SocketAddr>().unwrap(),
            bootstrap_peers: vec![],
            timeout: Duration::from_secs(60),
        };
        assert_eq!(config.t(), 1);
    }

    #[test]
    fn test_t_with_large_threshold() {
        let identity_key = ed25519::PrivateKey::from_seed(42);
        let participants: Vec<_> =
            (0..100).map(|i| ed25519::PrivateKey::from_seed(i as u64).public_key()).collect();

        let config = DkgConfig {
            identity_key,
            validator_index: 0,
            participants,
            threshold: 67,
            chain_id: 1337,
            data_dir: PathBuf::from("/tmp/dkg-test"),
            listen_addr: "127.0.0.1:8000".parse::<SocketAddr>().unwrap(),
            bootstrap_peers: vec![],
            timeout: Duration::from_secs(60),
        };
        assert_eq!(config.t(), 67);
    }

    #[test]
    fn test_my_public_key_derived_from_identity() {
        let config = test_config();
        let expected_public_key = config.identity_key.public_key();
        let actual_public_key = config.my_public_key();
        assert_eq!(actual_public_key, expected_public_key);
    }

    #[test]
    fn test_my_public_key_consistent() {
        let config = test_config();
        let first_call = config.my_public_key();
        let second_call = config.my_public_key();
        assert_eq!(first_call, second_call);
    }

    #[test]
    fn test_my_public_key_different_identities() {
        let identity_key1 = ed25519::PrivateKey::from_seed(42);
        let identity_key2 = ed25519::PrivateKey::from_seed(43);

        let config1 = DkgConfig {
            identity_key: identity_key1,
            validator_index: 0,
            participants: vec![
                ed25519::PrivateKey::from_seed(42).public_key(),
                ed25519::PrivateKey::from_seed(43).public_key(),
            ],
            threshold: 2,
            chain_id: 1337,
            data_dir: PathBuf::from("/tmp/dkg-test-1"),
            listen_addr: "127.0.0.1:8001".parse::<SocketAddr>().unwrap(),
            bootstrap_peers: vec![],
            timeout: Duration::from_secs(60),
        };

        let config2 = DkgConfig {
            identity_key: identity_key2,
            validator_index: 1,
            participants: vec![
                ed25519::PrivateKey::from_seed(42).public_key(),
                ed25519::PrivateKey::from_seed(43).public_key(),
            ],
            threshold: 2,
            chain_id: 1337,
            data_dir: PathBuf::from("/tmp/dkg-test-2"),
            listen_addr: "127.0.0.1:8002".parse::<SocketAddr>().unwrap(),
            bootstrap_peers: vec![],
            timeout: Duration::from_secs(60),
        };

        assert_ne!(config1.my_public_key(), config2.my_public_key());
    }

    #[test]
    fn test_dkg_config_debug_implementation() {
        let config = test_config();
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("DkgConfig"));
    }

    #[test]
    fn test_dkg_config_clone() {
        let config = test_config();
        let cloned = config.clone();

        assert_eq!(config.n(), cloned.n());
        assert_eq!(config.t(), cloned.t());
        assert_eq!(config.my_public_key(), cloned.my_public_key());
        assert_eq!(config.chain_id, cloned.chain_id);
        assert_eq!(config.validator_index, cloned.validator_index);
    }

    #[test]
    fn test_dkg_config_participants_matches_public_keys() {
        let config = test_config();
        assert_eq!(config.participants.len(), 4);
        assert_eq!(config.participants.len(), config.n());
    }

    #[test]
    fn test_dkg_config_threshold_boundary() {
        let identity_key = ed25519::PrivateKey::from_seed(42);
        let participants: Vec<_> =
            (0..4).map(|i| ed25519::PrivateKey::from_seed(i as u64).public_key()).collect();

        let config = DkgConfig {
            identity_key,
            validator_index: 0,
            participants,
            threshold: 4,
            chain_id: 1337,
            data_dir: PathBuf::from("/tmp/dkg-test"),
            listen_addr: "127.0.0.1:8000".parse::<SocketAddr>().unwrap(),
            bootstrap_peers: vec![],
            timeout: Duration::from_secs(60),
        };

        assert_eq!(config.t(), config.n() as u32);
    }
}
