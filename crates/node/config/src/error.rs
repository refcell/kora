//! Configuration error types.

use std::path::PathBuf;

/// Errors that can occur when loading or parsing configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Failed to read configuration file.
    #[error("failed to read config file {path}: {source}")]
    Read {
        /// The path that failed to read.
        path: PathBuf,
        /// The underlying IO error.
        source: std::io::Error,
    },

    /// Failed to parse TOML configuration.
    #[error("failed to parse TOML config: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// Failed to parse JSON configuration.
    #[error("failed to parse JSON config: {0}")]
    JsonParse(#[from] serde_json::Error),

    /// Failed to serialize configuration to TOML.
    #[error("failed to serialize config to TOML: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    /// Invalid validator key format.
    #[error("invalid validator key format: expected 32 bytes, got {0}")]
    InvalidKeyLength(usize),

    /// Failed to write file.
    #[error("failed to write {path}: {source}")]
    Write {
        /// Path.
        path: PathBuf,
        /// IO error.
        source: std::io::Error,
    },

    /// Failed to create directory.
    #[error("failed to create directory {path}: {source}")]
    CreateDir {
        /// Path.
        path: PathBuf,
        /// IO error.
        source: std::io::Error,
    },

    /// Invalid participant public key length.
    #[error("invalid participant public key: expected 32 bytes, got {0}")]
    InvalidParticipantKeyLength(usize),

    /// Failed to parse participant public key.
    #[error("invalid participant public key bytes")]
    InvalidParticipantKey,
}

#[cfg(test)]
mod tests {
    use std::io::{Error as IoError, ErrorKind};

    use super::*;

    #[test]
    fn test_read_error_display() {
        let err = ConfigError::Read {
            path: PathBuf::from("/etc/kora/config.toml"),
            source: IoError::new(ErrorKind::NotFound, "file not found"),
        };
        let display = err.to_string();
        assert!(display.contains("failed to read config file"));
        assert!(display.contains("/etc/kora/config.toml"));
        assert!(display.contains("file not found"));
    }

    #[test]
    fn test_write_error_display() {
        let err = ConfigError::Write {
            path: PathBuf::from("/var/lib/kora/state.db"),
            source: IoError::new(ErrorKind::PermissionDenied, "permission denied"),
        };
        let display = err.to_string();
        assert!(display.contains("failed to write"));
        assert!(display.contains("/var/lib/kora/state.db"));
        assert!(display.contains("permission denied"));
    }

    #[test]
    fn test_create_dir_error_display() {
        let err = ConfigError::CreateDir {
            path: PathBuf::from("/var/lib/kora"),
            source: IoError::new(ErrorKind::AlreadyExists, "directory exists"),
        };
        let display = err.to_string();
        assert!(display.contains("failed to create directory"));
        assert!(display.contains("/var/lib/kora"));
        assert!(display.contains("directory exists"));
    }

    #[test]
    fn test_invalid_key_length_display() {
        let err = ConfigError::InvalidKeyLength(16);
        assert_eq!(err.to_string(), "invalid validator key format: expected 32 bytes, got 16");
    }

    #[test]
    fn test_invalid_participant_key_length_display() {
        let err = ConfigError::InvalidParticipantKeyLength(64);
        assert_eq!(err.to_string(), "invalid participant public key: expected 32 bytes, got 64");
    }

    #[test]
    fn test_invalid_participant_key_display() {
        let err = ConfigError::InvalidParticipantKey;
        assert_eq!(err.to_string(), "invalid participant public key bytes");
    }

    #[test]
    fn test_toml_parse_error_from() {
        let result: Result<toml::Value, _> = toml::from_str("invalid = [unclosed");
        let toml_err = result.unwrap_err();
        let config_err: ConfigError = toml_err.into();
        let display = config_err.to_string();
        assert!(display.contains("failed to parse TOML config"));
    }

    #[test]
    fn test_json_parse_error_from() {
        let result: Result<serde_json::Value, _> = serde_json::from_str("{invalid}");
        let json_err = result.unwrap_err();
        let config_err: ConfigError = json_err.into();
        let display = config_err.to_string();
        assert!(display.contains("failed to parse JSON config"));
    }

    #[test]
    fn test_config_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ConfigError>();
    }

    #[test]
    fn test_config_error_debug() {
        let err = ConfigError::InvalidKeyLength(24);
        let debug = format!("{:?}", err);
        assert!(debug.contains("InvalidKeyLength"));
        assert!(debug.contains("24"));
    }
}
