//! Network configuration.

use serde::{Deserialize, Serialize};

/// Default listen address.
pub const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:30303";

/// Network layer configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkConfig {
    /// Address to listen for P2P connections.
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    /// External address for NAT traversal (if different from listen_addr).
    /// Use this when behind NAT/firewall to specify the publicly reachable address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialable_addr: Option<String>,

    /// Bootstrap peers to connect to on startup.
    #[serde(default)]
    pub bootstrap_peers: Vec<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: DEFAULT_LISTEN_ADDR.to_string(),
            dialable_addr: None,
            bootstrap_peers: Vec::new(),
        }
    }
}

fn default_listen_addr() -> String {
    DEFAULT_LISTEN_ADDR.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_network_config() {
        let config = NetworkConfig::default();
        assert_eq!(config.listen_addr, DEFAULT_LISTEN_ADDR);
        assert!(config.dialable_addr.is_none());
        assert!(config.bootstrap_peers.is_empty());
    }

    #[test]
    fn test_network_config_serde_roundtrip() {
        let config = NetworkConfig {
            listen_addr: "127.0.0.1:9000".to_string(),
            dialable_addr: Some("1.2.3.4:9000".to_string()),
            bootstrap_peers: vec!["peer1:30303".to_string()],
        };
        let serialized = serde_json::to_string(&config).expect("serialize");
        let deserialized: NetworkConfig = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_network_config_toml_roundtrip() {
        let config = NetworkConfig {
            listen_addr: "0.0.0.0:8080".to_string(),
            dialable_addr: None,
            bootstrap_peers: vec!["node1.example.com:30303".to_string()],
        };
        let serialized = toml::to_string(&config).expect("serialize toml");
        let deserialized: NetworkConfig = toml::from_str(&serialized).expect("deserialize toml");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_network_config_serde_defaults() {
        let config: NetworkConfig = serde_json::from_str("{}").expect("deserialize");
        assert_eq!(config.listen_addr, DEFAULT_LISTEN_ADDR);
        assert!(config.dialable_addr.is_none());
        assert!(config.bootstrap_peers.is_empty());
    }

    #[test]
    fn test_network_config_dialable_addr_skip_serializing_when_none() {
        let config = NetworkConfig::default();
        let serialized = serde_json::to_string(&config).expect("serialize");
        assert!(!serialized.contains("dialable_addr"));
    }

    #[test]
    fn test_network_config_dialable_addr_serialized_when_some() {
        let config = NetworkConfig {
            dialable_addr: Some("1.2.3.4:30303".to_string()),
            ..Default::default()
        };
        let serialized = serde_json::to_string(&config).expect("serialize");
        assert!(serialized.contains("dialable_addr"));
    }

    #[test]
    fn test_network_config_clone_and_eq() {
        let config = NetworkConfig {
            listen_addr: "10.0.0.1:5555".to_string(),
            dialable_addr: Some("external.host:5555".to_string()),
            bootstrap_peers: vec!["a".to_string()],
        };
        assert_eq!(config, config.clone());
        assert_ne!(config, NetworkConfig::default());
    }
}
