use thiserror::Error;

/// Errors that can occur during Distributed Key Generation (DKG) ceremonies.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum DkgError {
    /// The DKG ceremony failed with a message.
    #[error("DKG ceremony failed: {0}")]
    CeremonyFailed(String),

    /// Participant count mismatch.
    #[error("Invalid participant count: expected {expected}, got {actual}")]
    InvalidParticipantCount {
        /// Expected count.
        expected: usize,
        /// Actual count.
        actual: usize,
    },

    /// Invalid threshold for the given participant count.
    #[error("Threshold {threshold} is invalid for {participants} participants")]
    InvalidThreshold {
        /// Threshold value.
        threshold: u32,
        /// Number of participants.
        participants: usize,
    },

    /// Network communication error.
    #[error("Network error: {0}")]
    Network(String),

    /// Cryptographic operation error.
    #[error("Cryptographic error: {0}")]
    Crypto(String),

    /// Serialization or deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Timeout waiting for DKG completion.
    #[error("Timeout waiting for DKG completion")]
    Timeout,

    /// Missing share for a participant.
    #[error("Missing share for participant")]
    MissingShare,

    /// Invalid dealer log received.
    #[error("Invalid dealer log from {dealer}")]
    InvalidDealerLog {
        /// Dealer identifier.
        dealer: String,
    },

    /// Message from unknown sender.
    #[error("Unknown sender: {sender}")]
    UnknownSender {
        /// Sender identifier.
        sender: String,
    },

    /// Sender identity mismatch.
    #[error("Sender mismatch: expected {expected}, got {actual}")]
    SenderMismatch {
        /// Expected sender.
        expected: String,
        /// Actual sender.
        actual: String,
    },

    /// Unauthorized sender for the message type.
    #[error("Unauthorized sender: only leader can send AllLogs")]
    UnauthorizedSender,

    /// Duplicate message from a dealer.
    #[error("Duplicate message from dealer: {dealer}")]
    DuplicateDealer {
        /// Dealer identifier.
        dealer: String,
    },

    /// Too many dealers exceed the maximum.
    #[error("Too many dealers: {count} exceeds maximum {max}")]
    TooManyDealers {
        /// Current count.
        count: usize,
        /// Maximum allowed.
        max: usize,
    },

    /// Session ID mismatch.
    #[error("Session mismatch: expected {expected}, received {received}")]
    SessionMismatch {
        /// Expected session.
        expected: String,
        /// Received session.
        received: String,
    },

    /// Invalid message format or content.
    #[error("Invalid message: {0}")]
    InvalidMessage(String),
}
