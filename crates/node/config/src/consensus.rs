//! Consensus configuration.

use std::path::PathBuf;

use alloy_primitives::hex;
use commonware_codec::{FixedSize, ReadExt};
use commonware_cryptography::ed25519;
use serde::{Deserialize, Serialize};

use crate::ConfigError;

/// Default validator threshold.
pub const DEFAULT_THRESHOLD: u32 = 2;

/// Consensus layer configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsensusConfig {
    /// Path to the validator key file.
    #[serde(default)]
    pub validator_key: Option<PathBuf>,

    /// Threshold for consensus (e.g., 2f+1 of 3f+1).
    #[serde(default = "default_threshold")]
    pub threshold: u32,

    /// List of participant public keys (hex-encoded).
    #[serde(
        default,
        serialize_with = "serialize_participants",
        deserialize_with = "deserialize_participants"
    )]
    pub participants: Vec<Vec<u8>>,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self { validator_key: None, threshold: DEFAULT_THRESHOLD, participants: Vec::new() }
    }
}

impl ConsensusConfig {
    /// Build the validator set from configured participants.
    ///
    /// Parses the hex-encoded participant public keys into [`ed25519::PublicKey`] values.
    /// Returns an empty set if no participants are configured.
    pub fn build_validator_set(&self) -> Result<Vec<ed25519::PublicKey>, ConfigError> {
        self.participants
            .iter()
            .map(|bytes| {
                if bytes.len() != ed25519::PublicKey::SIZE {
                    return Err(ConfigError::InvalidParticipantKeyLength(bytes.len()));
                }
                let mut buf = bytes.as_slice();
                ed25519::PublicKey::read(&mut buf).map_err(|_| ConfigError::InvalidParticipantKey)
            })
            .collect()
    }
}

const fn default_threshold() -> u32 {
    DEFAULT_THRESHOLD
}

fn serialize_participants<S>(participants: &[Vec<u8>], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeSeq;
    let mut seq = serializer.serialize_seq(Some(participants.len()))?;
    for p in participants {
        seq.serialize_element(&hex::encode(p))?;
    }
    seq.end()
}

fn deserialize_participants<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let strings: Vec<String> = Vec::deserialize(deserializer)?;
    strings
        .into_iter()
        .map(|s| hex::decode(s.strip_prefix("0x").unwrap_or(&s)).map_err(serde::de::Error::custom))
        .collect()
}

#[cfg(test)]
mod tests {
    use commonware_codec::Write as _;
    use commonware_cryptography::Signer as _;

    use super::*;

    fn create_valid_public_key_bytes() -> Vec<u8> {
        let private_key =
            ed25519::PrivateKey::from(ed25519_consensus::SigningKey::from([42u8; 32]));
        let public_key = private_key.public_key();
        let mut bytes = Vec::new();
        public_key.write(&mut bytes);
        bytes
    }

    #[test]
    fn default_consensus_config() {
        let config = ConsensusConfig::default();
        assert!(config.validator_key.is_none());
        assert_eq!(config.threshold, DEFAULT_THRESHOLD);
        assert!(config.participants.is_empty());
    }

    #[test]
    fn default_threshold_constant() {
        assert_eq!(DEFAULT_THRESHOLD, 2);
        assert_eq!(default_threshold(), DEFAULT_THRESHOLD);
    }

    #[test]
    fn serde_json_roundtrip() {
        let pk_bytes = create_valid_public_key_bytes();
        let config = ConsensusConfig {
            validator_key: Some(PathBuf::from("/path/to/key")),
            threshold: 3,
            participants: vec![pk_bytes],
        };
        let serialized = serde_json::to_string(&config).expect("serialize");
        let deserialized: ConsensusConfig = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn serde_toml_roundtrip() {
        let config =
            ConsensusConfig { validator_key: Some("/path/to/key".into()), ..Default::default() };
        let serialized = toml::to_string(&config).expect("serialize toml");
        let deserialized: ConsensusConfig = toml::from_str(&serialized).expect("deserialize toml");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn serde_defaults_applied() {
        let config: ConsensusConfig = serde_json::from_str("{}").expect("deserialize");
        assert!(config.validator_key.is_none());
        assert_eq!(config.threshold, DEFAULT_THRESHOLD);
        assert!(config.participants.is_empty());
    }

    #[test]
    fn serde_partial_threshold() {
        let config: ConsensusConfig =
            serde_json::from_str(r#"{"threshold": 7}"#).expect("deserialize");
        assert_eq!(config.threshold, 7);
        assert!(config.validator_key.is_none());
        assert!(config.participants.is_empty());
    }

    #[test]
    fn serde_partial_validator_key() {
        let config: ConsensusConfig =
            serde_json::from_str(r#"{"validator_key": "/etc/key"}"#).expect("deserialize");
        assert_eq!(config.validator_key, Some(PathBuf::from("/etc/key")));
        assert_eq!(config.threshold, DEFAULT_THRESHOLD);
    }

    #[test]
    fn deserialize_participants_with_0x_prefix() {
        let pk_bytes = create_valid_public_key_bytes();
        let hex_with_prefix = format!("0x{}", hex::encode(&pk_bytes));
        let json = format!(r#"{{"participants": ["{}"]}}"#, hex_with_prefix);

        let config: ConsensusConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config.participants.len(), 1);
        assert_eq!(config.participants[0], pk_bytes);
    }

    #[test]
    fn deserialize_participants_without_prefix() {
        let pk_bytes = create_valid_public_key_bytes();
        let hex_without_prefix = hex::encode(&pk_bytes);
        let json = format!(r#"{{"participants": ["{}"]}}"#, hex_without_prefix);

        let config: ConsensusConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config.participants.len(), 1);
        assert_eq!(config.participants[0], pk_bytes);
    }

    #[test]
    fn build_validator_set_empty() {
        let config = ConsensusConfig::default();
        let result = config.build_validator_set().expect("build empty set");
        assert!(result.is_empty());
    }

    #[test]
    fn build_validator_set_single_key() {
        let pk_bytes = create_valid_public_key_bytes();
        let config = ConsensusConfig { participants: vec![pk_bytes], ..Default::default() };
        let result = config.build_validator_set().expect("build validator set");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn build_validator_set_multiple_keys() {
        let keys: Vec<_> = (1..=3u8)
            .map(|i| {
                let pk = ed25519::PrivateKey::from(ed25519_consensus::SigningKey::from([i; 32]));
                let mut bytes = Vec::new();
                pk.public_key().write(&mut bytes);
                bytes
            })
            .collect();

        let config = ConsensusConfig { participants: keys, threshold: 2, ..Default::default() };

        let result = config.build_validator_set().expect("build validator set");
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn build_validator_set_invalid_length() {
        let config = ConsensusConfig { participants: vec![vec![0u8; 16]], ..Default::default() };
        let result = config.build_validator_set();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::InvalidParticipantKeyLength(16)));
    }

    #[test]
    fn participants_hex_serialization() {
        let pk_bytes = create_valid_public_key_bytes();
        let expected_hex = hex::encode(&pk_bytes);
        let config = ConsensusConfig { participants: vec![pk_bytes], ..Default::default() };

        let serialized = serde_json::to_string(&config).expect("serialize");
        assert!(serialized.contains(&expected_hex));
    }

    #[test]
    fn consensus_config_clone_and_eq() {
        let pk_bytes = create_valid_public_key_bytes();
        let config = ConsensusConfig {
            validator_key: Some(PathBuf::from("/custom/path")),
            threshold: 10,
            participants: vec![pk_bytes],
        };
        assert_eq!(config, config.clone());
        assert_ne!(config, ConsensusConfig::default());
    }

    #[test]
    fn consensus_config_debug() {
        let config = ConsensusConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("ConsensusConfig"));
        assert!(debug.contains("threshold"));
    }
}
