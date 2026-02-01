//! Execution configuration.

use serde::{Deserialize, Serialize};

/// Default gas limit per block.
pub const DEFAULT_GAS_LIMIT: u64 = 30_000_000;

/// Default block time in seconds.
pub const DEFAULT_BLOCK_TIME: u64 = 2;

/// Execution layer configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionConfig {
    /// Maximum gas per block.
    #[serde(default = "default_gas_limit")]
    pub gas_limit: u64,

    /// Target block time in seconds.
    #[serde(default = "default_block_time")]
    pub block_time: u64,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self { gas_limit: DEFAULT_GAS_LIMIT, block_time: DEFAULT_BLOCK_TIME }
    }
}

const fn default_gas_limit() -> u64 {
    DEFAULT_GAS_LIMIT
}

const fn default_block_time() -> u64 {
    DEFAULT_BLOCK_TIME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_execution_config() {
        let config = ExecutionConfig::default();
        assert_eq!(config.gas_limit, DEFAULT_GAS_LIMIT);
        assert_eq!(config.block_time, DEFAULT_BLOCK_TIME);
    }

    #[test]
    fn test_execution_config_serde_roundtrip() {
        let config = ExecutionConfig { gas_limit: 50_000_000, block_time: 5 };
        let serialized = serde_json::to_string(&config).expect("serialize");
        let deserialized: ExecutionConfig = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_execution_config_toml_roundtrip() {
        let config = ExecutionConfig { gas_limit: 15_000_000, block_time: 1 };
        let serialized = toml::to_string(&config).expect("serialize toml");
        let deserialized: ExecutionConfig = toml::from_str(&serialized).expect("deserialize toml");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_execution_config_serde_defaults() {
        let config: ExecutionConfig = serde_json::from_str("{}").expect("deserialize");
        assert_eq!(config.gas_limit, DEFAULT_GAS_LIMIT);
        assert_eq!(config.block_time, DEFAULT_BLOCK_TIME);
    }

    #[test]
    fn test_execution_config_partial_defaults() {
        let config: ExecutionConfig =
            serde_json::from_str(r#"{"gas_limit": 10000000}"#).expect("deserialize");
        assert_eq!(config.gas_limit, 10_000_000);
        assert_eq!(config.block_time, DEFAULT_BLOCK_TIME);

        let config: ExecutionConfig =
            serde_json::from_str(r#"{"block_time": 10}"#).expect("deserialize");
        assert_eq!(config.gas_limit, DEFAULT_GAS_LIMIT);
        assert_eq!(config.block_time, 10);
    }

    #[test]
    fn test_execution_config_clone_and_eq() {
        let config = ExecutionConfig { gas_limit: 999, block_time: 42 };
        assert_eq!(config, config.clone());
        assert_ne!(config, ExecutionConfig::default());
    }
}
