use thiserror::Error;

/// Errors that can occur during Distributed Key Generation (DKG) ceremonies.
#[derive(Debug, Error)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ceremony_failed_display() {
        let err = DkgError::CeremonyFailed("test failure".to_string());
        assert_eq!(err.to_string(), "DKG ceremony failed: test failure");
    }

    #[test]
    fn test_invalid_participant_count_display() {
        let err = DkgError::InvalidParticipantCount { expected: 5, actual: 3 };
        assert_eq!(err.to_string(), "Invalid participant count: expected 5, got 3");
    }

    #[test]
    fn test_invalid_threshold_display() {
        let err = DkgError::InvalidThreshold { threshold: 10, participants: 5 };
        assert_eq!(err.to_string(), "Threshold 10 is invalid for 5 participants");
    }

    #[test]
    fn test_network_display() {
        let err = DkgError::Network("connection refused".to_string());
        assert_eq!(err.to_string(), "Network error: connection refused");
    }

    #[test]
    fn test_crypto_display() {
        let err = DkgError::Crypto("invalid signature".to_string());
        assert_eq!(err.to_string(), "Cryptographic error: invalid signature");
    }

    #[test]
    fn test_serialization_display() {
        let err = DkgError::Serialization("invalid json".to_string());
        assert_eq!(err.to_string(), "Serialization error: invalid json");
    }

    #[test]
    fn test_io_error_from() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: DkgError = io_err.into();
        assert!(err.to_string().contains("IO error"));
    }

    #[test]
    fn test_timeout_display() {
        let err = DkgError::Timeout;
        assert_eq!(err.to_string(), "Timeout waiting for DKG completion");
    }

    #[test]
    fn test_missing_share_display() {
        let err = DkgError::MissingShare;
        assert_eq!(err.to_string(), "Missing share for participant");
    }

    #[test]
    fn test_invalid_dealer_log_display() {
        let err = DkgError::InvalidDealerLog { dealer: "node1".to_string() };
        assert_eq!(err.to_string(), "Invalid dealer log from node1");
    }

    #[test]
    fn test_unknown_sender_display() {
        let err = DkgError::UnknownSender { sender: "unknown_node".to_string() };
        assert_eq!(err.to_string(), "Unknown sender: unknown_node");
    }

    #[test]
    fn test_sender_mismatch_display() {
        let err =
            DkgError::SenderMismatch { expected: "node1".to_string(), actual: "node2".to_string() };
        assert_eq!(err.to_string(), "Sender mismatch: expected node1, got node2");
    }

    #[test]
    fn test_unauthorized_sender_display() {
        let err = DkgError::UnauthorizedSender;
        assert_eq!(err.to_string(), "Unauthorized sender: only leader can send AllLogs");
    }

    #[test]
    fn test_duplicate_dealer_display() {
        let err = DkgError::DuplicateDealer { dealer: "dealer1".to_string() };
        assert_eq!(err.to_string(), "Duplicate message from dealer: dealer1");
    }

    #[test]
    fn test_too_many_dealers_display() {
        let err = DkgError::TooManyDealers { count: 15, max: 10 };
        assert_eq!(err.to_string(), "Too many dealers: 15 exceeds maximum 10");
    }

    #[test]
    fn test_session_mismatch_display() {
        let err = DkgError::SessionMismatch {
            expected: "session1".to_string(),
            received: "session2".to_string(),
        };
        assert_eq!(err.to_string(), "Session mismatch: expected session1, received session2");
    }

    #[test]
    fn test_invalid_message_display() {
        let err = DkgError::InvalidMessage("malformed data".to_string());
        assert_eq!(err.to_string(), "Invalid message: malformed data");
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DkgError>();
    }

    #[test]
    fn test_error_debug_impl() {
        let err = DkgError::Timeout;
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Timeout"));
    }
}
