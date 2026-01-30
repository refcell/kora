//! DKG state persistence for crash recovery.

use std::{collections::BTreeMap, path::Path};

use serde::{Deserialize, Serialize};

use crate::{CeremonySession, DkgError};

/// Current phase of the DKG protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DkgPhase {
    /// Waiting for ceremony to start.
    AwaitingStart,
    /// Dealer has been started (commitments/shares generated).
    DealerStarted,
    /// Collecting messages from other participants.
    CollectingMessages,
    /// Dealer has been finalized (signed log created).
    DealerFinalized,
    /// Collecting dealer logs from other participants.
    CollectingLogs,
    /// DKG completed successfully.
    Completed,
    /// DKG failed.
    Failed,
}

impl std::fmt::Display for DkgPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AwaitingStart => write!(f, "AwaitingStart"),
            Self::DealerStarted => write!(f, "DealerStarted"),
            Self::CollectingMessages => write!(f, "CollectingMessages"),
            Self::DealerFinalized => write!(f, "DealerFinalized"),
            Self::CollectingLogs => write!(f, "CollectingLogs"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

/// Serializable ceremony session for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializedSession {
    ceremony_id: String,
    chain_id: u64,
    round: u32,
}

impl From<&CeremonySession> for SerializedSession {
    fn from(session: &CeremonySession) -> Self {
        Self {
            ceremony_id: hex::encode(session.ceremony_id),
            chain_id: session.chain_id,
            round: session.round,
        }
    }
}

impl TryFrom<SerializedSession> for CeremonySession {
    type Error = DkgError;

    fn try_from(s: SerializedSession) -> Result<Self, Self::Error> {
        let ceremony_id_bytes =
            hex::decode(&s.ceremony_id).map_err(|e| DkgError::Serialization(e.to_string()))?;
        if ceremony_id_bytes.len() != 32 {
            return Err(DkgError::Serialization("Invalid ceremony_id length".into()));
        }
        let mut ceremony_id = [0u8; 32];
        ceremony_id.copy_from_slice(&ceremony_id_bytes);
        Ok(Self { ceremony_id, chain_id: s.chain_id, round: s.round })
    }
}

/// Persisted DKG state for crash recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedDkgState {
    /// Current phase of the DKG protocol.
    pub phase: DkgPhase,
    /// Session metadata (serialized).
    session: SerializedSession,
    /// Whether the dealer has been started.
    pub dealer_started: bool,
    /// Whether the dealer has been finalized.
    pub dealer_finalized: bool,
    /// Our signed dealer log (serialized bytes as hex).
    pub our_signed_log: Option<String>,
    /// Received dealer logs from other participants (hex pk -> hex serialized log).
    pub received_logs: BTreeMap<String, String>,
    /// Timestamp when this state was created (nanos since epoch).
    pub timestamp: u64,
}

impl PersistedDkgState {
    const STATE_FILE: &'static str = "dkg_state.json";

    /// Create a new persisted state.
    pub fn new(session: &CeremonySession, timestamp: u64) -> Self {
        Self {
            phase: DkgPhase::AwaitingStart,
            session: session.into(),
            dealer_started: false,
            dealer_finalized: false,
            our_signed_log: None,
            received_logs: BTreeMap::new(),
            timestamp,
        }
    }

    /// Get the ceremony session.
    pub fn session(&self) -> Result<CeremonySession, DkgError> {
        self.session.clone().try_into()
    }

    /// Update the session.
    pub fn set_session(&mut self, session: &CeremonySession) {
        self.session = session.into();
    }

    /// Save state to disk.
    pub fn save(&self, data_dir: &Path) -> Result<(), DkgError> {
        let path = data_dir.join(Self::STATE_FILE);
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Load state from disk.
    pub fn load(data_dir: &Path) -> Result<Self, DkgError> {
        let path = data_dir.join(Self::STATE_FILE);
        let content = std::fs::read_to_string(&path)?;
        let state: Self =
            serde_json::from_str(&content).map_err(|e| DkgError::Serialization(e.to_string()))?;
        Ok(state)
    }

    /// Check if state file exists.
    pub fn exists(data_dir: &Path) -> bool {
        data_dir.join(Self::STATE_FILE).exists()
    }

    /// Clear state file from disk.
    pub fn clear(data_dir: &Path) -> Result<(), DkgError> {
        let path = data_dir.join(Self::STATE_FILE);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Set our signed dealer log.
    pub fn set_our_signed_log(&mut self, log_bytes: Vec<u8>) {
        self.our_signed_log = Some(hex::encode(log_bytes));
    }

    /// Get our signed dealer log bytes.
    pub fn get_our_signed_log(&self) -> Option<Vec<u8>> {
        self.our_signed_log.as_ref().and_then(|s| hex::decode(s).ok())
    }

    /// Add a received dealer log.
    pub fn add_received_log(&mut self, pk_hex: String, log_bytes: Vec<u8>) {
        self.received_logs.insert(pk_hex, hex::encode(log_bytes));
    }

    /// Get received logs as bytes.
    pub fn get_received_logs(&self) -> BTreeMap<String, Vec<u8>> {
        self.received_logs
            .iter()
            .filter_map(|(k, v)| hex::decode(v).ok().map(|bytes| (k.clone(), bytes)))
            .collect()
    }
}
