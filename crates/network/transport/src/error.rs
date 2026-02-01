//! Transport error types.

/// Errors that can occur during transport operations.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// Failed to parse listen address.
    #[error("invalid listen address: {0}")]
    InvalidListenAddr(String),

    /// Failed to parse dialable address.
    #[error("invalid dialable address: {0}")]
    InvalidDialableAddr(String),

    /// Failed to parse bootstrap peer.
    #[error("invalid bootstrap peer format '{0}': expected 'PUBLIC_KEY_HEX@HOST:PORT'")]
    InvalidBootstrapPeer(String),

    /// Failed to parse public key from hex.
    #[error("invalid public key hex: {0}")]
    InvalidPublicKeyHex(String),

    /// Invalid public key length.
    #[error("invalid public key length: expected 32 bytes, got {0}")]
    InvalidPublicKeyLength(usize),

    /// Failed to parse public key.
    #[error("invalid public key bytes")]
    InvalidPublicKey,

    /// Failed to parse hostname.
    #[error("invalid hostname: {0}")]
    InvalidHostname(String),

    /// Failed to parse port.
    #[error("invalid port: {0}")]
    InvalidPort(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        assert_eq!(
            TransportError::InvalidListenAddr("bad".to_string()).to_string(),
            "invalid listen address: bad"
        );
        assert_eq!(
            TransportError::InvalidDialableAddr("bad".to_string()).to_string(),
            "invalid dialable address: bad"
        );
        assert!(TransportError::InvalidPublicKey.to_string().contains("public key"));
    }

    #[test]
    fn error_is_debug() {
        let err = TransportError::InvalidPublicKey;
        assert!(format!("{:?}", err).contains("InvalidPublicKey"));
    }
}
