//! Error types for simulation transport.

use thiserror::Error;

/// Errors that can occur when building simulation transport.
#[derive(Debug, Error)]
pub enum SimTransportError {
    /// Failed to register a channel.
    #[error("channel registration failed: {0}")]
    ChannelRegistration(String),
}
