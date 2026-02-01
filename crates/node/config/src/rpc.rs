//! RPC configuration.

use serde::{Deserialize, Serialize};

/// Default HTTP RPC address.
pub const DEFAULT_HTTP_ADDR: &str = "0.0.0.0:8545";

/// Default WebSocket RPC address.
pub const DEFAULT_WS_ADDR: &str = "0.0.0.0:8546";

/// RPC server configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpcConfig {
    /// HTTP JSON-RPC server address.
    #[serde(default = "default_http_addr")]
    pub http_addr: String,

    /// WebSocket server address.
    #[serde(default = "default_ws_addr")]
    pub ws_addr: String,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self { http_addr: DEFAULT_HTTP_ADDR.to_string(), ws_addr: DEFAULT_WS_ADDR.to_string() }
    }
}

fn default_http_addr() -> String {
    DEFAULT_HTTP_ADDR.to_string()
}

fn default_ws_addr() -> String {
    DEFAULT_WS_ADDR.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_rpc_config() {
        let config = RpcConfig::default();
        assert_eq!(config.http_addr, DEFAULT_HTTP_ADDR);
        assert_eq!(config.ws_addr, DEFAULT_WS_ADDR);
    }

    #[test]
    fn test_rpc_config_serde_roundtrip() {
        let config = RpcConfig {
            http_addr: "127.0.0.1:8080".to_string(),
            ws_addr: "127.0.0.1:8081".to_string(),
        };
        let serialized = serde_json::to_string(&config).expect("serialize");
        let deserialized: RpcConfig = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_rpc_config_toml_roundtrip() {
        let config = RpcConfig {
            http_addr: "0.0.0.0:9545".to_string(),
            ws_addr: "0.0.0.0:9546".to_string(),
        };
        let serialized = toml::to_string(&config).expect("serialize toml");
        let deserialized: RpcConfig = toml::from_str(&serialized).expect("deserialize toml");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_rpc_config_serde_defaults() {
        let config: RpcConfig = serde_json::from_str("{}").expect("deserialize");
        assert_eq!(config.http_addr, DEFAULT_HTTP_ADDR);
        assert_eq!(config.ws_addr, DEFAULT_WS_ADDR);
    }

    #[test]
    fn test_rpc_config_partial_defaults() {
        let config: RpcConfig =
            serde_json::from_str(r#"{"http_addr": "1.2.3.4:8545"}"#).expect("deserialize");
        assert_eq!(config.http_addr, "1.2.3.4:8545");
        assert_eq!(config.ws_addr, DEFAULT_WS_ADDR);

        let config: RpcConfig =
            serde_json::from_str(r#"{"ws_addr": "5.6.7.8:8546"}"#).expect("deserialize");
        assert_eq!(config.http_addr, DEFAULT_HTTP_ADDR);
        assert_eq!(config.ws_addr, "5.6.7.8:8546");
    }

    #[test]
    fn test_rpc_config_clone_and_eq() {
        let config =
            RpcConfig { http_addr: "custom:1111".to_string(), ws_addr: "custom:2222".to_string() };
        assert_eq!(config, config.clone());
        assert_ne!(config, RpcConfig::default());
    }
}
