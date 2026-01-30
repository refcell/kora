use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::DkgError;

/// Output of a successful DKG ceremony containing the group key, shares, and participant info.
#[derive(Debug, Clone)]
pub struct DkgOutput {
    /// The aggregated group public key derived from all participants' contributions.
    pub group_public_key: Vec<u8>,
    /// Coefficients of the public polynomial used for share verification.
    pub public_polynomial: Vec<u8>,
    /// Minimum number of participants required to reconstruct the secret.
    pub threshold: u32,
    /// Total number of participants in the DKG ceremony.
    pub participants: usize,
    /// This participant's index in the DKG ceremony (1-indexed).
    pub share_index: u32,
    /// This participant's secret share of the distributed key.
    pub share_secret: Vec<u8>,
    /// Public keys of all participants in the DKG ceremony.
    pub participant_keys: Vec<Vec<u8>>,
}

#[derive(Serialize, Deserialize)]
struct OutputJson {
    group_public_key: String,
    public_polynomial: String,
    threshold: u32,
    participants: usize,
    #[serde(default)]
    participant_keys: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct ShareJson {
    index: u32,
    secret: String,
}

impl DkgOutput {
    /// Persists the DKG output to `output.json` and the secret share to `share.key` in `data_dir`.
    pub fn save(&self, data_dir: &Path) -> Result<(), DkgError> {
        let output_json = OutputJson {
            group_public_key: hex::encode(&self.group_public_key),
            public_polynomial: hex::encode(&self.public_polynomial),
            threshold: self.threshold,
            participants: self.participants,
            participant_keys: self.participant_keys.iter().map(hex::encode).collect(),
        };

        let output_path = data_dir.join("output.json");
        std::fs::write(&output_path, serde_json::to_string_pretty(&output_json)?)?;

        let share_json =
            ShareJson { index: self.share_index, secret: hex::encode(&self.share_secret) };

        let share_path = data_dir.join("share.key");
        std::fs::write(&share_path, serde_json::to_string_pretty(&share_json)?)?;

        Ok(())
    }

    /// Loads a DKG output from `output.json` and `share.key` in `data_dir`.
    pub fn load(data_dir: &Path) -> Result<Self, DkgError> {
        let output_path = data_dir.join("output.json");
        let output_str = std::fs::read_to_string(&output_path)?;
        let output: OutputJson = serde_json::from_str(&output_str)
            .map_err(|e| DkgError::Serialization(e.to_string()))?;

        let share_path = data_dir.join("share.key");
        let share_str = std::fs::read_to_string(&share_path)?;
        let share: ShareJson =
            serde_json::from_str(&share_str).map_err(|e| DkgError::Serialization(e.to_string()))?;

        let participant_keys = output
            .participant_keys
            .iter()
            .map(|k| hex::decode(k).map_err(|e| DkgError::Serialization(e.to_string())))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            group_public_key: hex::decode(&output.group_public_key)
                .map_err(|e| DkgError::Serialization(e.to_string()))?,
            public_polynomial: hex::decode(&output.public_polynomial)
                .map_err(|e| DkgError::Serialization(e.to_string()))?,
            threshold: output.threshold,
            participants: output.participants,
            share_index: share.index,
            share_secret: hex::decode(&share.secret)
                .map_err(|e| DkgError::Serialization(e.to_string()))?,
            participant_keys,
        })
    }

    /// Returns `true` if both `output.json` and `share.key` exist in `data_dir`.
    pub fn exists(data_dir: &Path) -> bool {
        data_dir.join("output.json").exists() && data_dir.join("share.key").exists()
    }
}

impl From<serde_json::Error> for DkgError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e.to_string())
    }
}
