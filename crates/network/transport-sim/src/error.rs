//! Error types for simulation transport.

use thiserror::Error;

/// Errors that can occur when building simulation transport.
#[derive(Debug, Error)]
pub enum SimTransportError {
    /// Failed to register a channel.
    #[error("channel registration failed: {0}")]
    ChannelRegistration(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_registration_display() {
        let err = SimTransportError::ChannelRegistration("already exists".to_string());
        assert_eq!(err.to_string(), "channel registration failed: already exists");
    }

    #[test]
    fn test_error_debug() {
        let err = SimTransportError::ChannelRegistration("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("ChannelRegistration"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SimTransportError>();
    }
}
