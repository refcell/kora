use thiserror::Error;

#[derive(Debug, Error)]
pub enum DkgError {
    #[error("DKG ceremony failed: {0}")]
    CeremonyFailed(String),

    #[error("Invalid participant count: expected {expected}, got {actual}")]
    InvalidParticipantCount { expected: usize, actual: usize },

    #[error("Threshold {threshold} is invalid for {participants} participants")]
    InvalidThreshold { threshold: u32, participants: usize },

    #[error("Network error: {0}")]
    Network(String),

    #[error("Cryptographic error: {0}")]
    Crypto(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Timeout waiting for DKG completion")]
    Timeout,

    #[error("Missing share for participant")]
    MissingShare,

    #[error("Invalid dealer log from {dealer}")]
    InvalidDealerLog { dealer: String },

    #[error("Unknown sender: {sender}")]
    UnknownSender { sender: String },

    #[error("Sender mismatch: expected {expected}, got {actual}")]
    SenderMismatch { expected: String, actual: String },

    #[error("Unauthorized sender: only leader can send AllLogs")]
    UnauthorizedSender,

    #[error("Duplicate message from dealer: {dealer}")]
    DuplicateDealer { dealer: String },

    #[error("Too many dealers: {count} exceeds maximum {max}")]
    TooManyDealers { count: usize, max: usize },

    #[error("Session mismatch: expected {expected}, received {received}")]
    SessionMismatch { expected: String, received: String },

    #[error("Invalid message: {0}")]
    InvalidMessage(String),
}
