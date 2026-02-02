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

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn create_test_output() -> DkgOutput {
        DkgOutput {
            group_public_key: vec![0x01, 0x02, 0x03, 0x04],
            public_polynomial: vec![0x05, 0x06, 0x07, 0x08],
            threshold: 2,
            participants: 3,
            share_index: 1,
            share_secret: vec![0x09, 0x0a, 0x0b, 0x0c],
            participant_keys: vec![vec![0x0d, 0x0e], vec![0x0f, 0x10], vec![0x11, 0x12]],
        }
    }

    #[test]
    fn test_dkg_output_save_and_load_roundtrip() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let output = create_test_output();

        output.save(temp_dir.path()).expect("save output");

        assert!(temp_dir.path().join("output.json").exists());
        assert!(temp_dir.path().join("share.key").exists());

        let loaded = DkgOutput::load(temp_dir.path()).expect("load output");

        assert_eq!(loaded.group_public_key, output.group_public_key);
        assert_eq!(loaded.public_polynomial, output.public_polynomial);
        assert_eq!(loaded.threshold, output.threshold);
        assert_eq!(loaded.participants, output.participants);
        assert_eq!(loaded.share_index, output.share_index);
        assert_eq!(loaded.share_secret, output.share_secret);
        assert_eq!(loaded.participant_keys, output.participant_keys);
    }

    #[test]
    fn test_dkg_output_exists_true() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let output = create_test_output();
        output.save(temp_dir.path()).expect("save output");

        assert!(DkgOutput::exists(temp_dir.path()));
    }

    #[test]
    fn test_dkg_output_exists_false_empty_dir() {
        let temp_dir = TempDir::new().expect("create temp dir");
        assert!(!DkgOutput::exists(temp_dir.path()));
    }

    #[test]
    fn test_dkg_output_exists_false_missing_share_key() {
        let temp_dir = TempDir::new().expect("create temp dir");

        std::fs::write(
            temp_dir.path().join("output.json"),
            r#"{"group_public_key":"01020304","public_polynomial":"05060708","threshold":2,"participants":3,"participant_keys":[]}"#,
        )
        .expect("write output.json");

        assert!(!DkgOutput::exists(temp_dir.path()));
    }

    #[test]
    fn test_dkg_output_exists_false_missing_output_json() {
        let temp_dir = TempDir::new().expect("create temp dir");

        std::fs::write(temp_dir.path().join("share.key"), r#"{"index":1,"secret":"090a0b0c"}"#)
            .expect("write share.key");

        assert!(!DkgOutput::exists(temp_dir.path()));
    }

    #[test]
    fn test_dkg_output_load_missing_output_json() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let result = DkgOutput::load(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_dkg_output_load_invalid_output_json() {
        let temp_dir = TempDir::new().expect("create temp dir");

        std::fs::write(temp_dir.path().join("output.json"), "not valid json")
            .expect("write output.json");
        std::fs::write(temp_dir.path().join("share.key"), r#"{"index":1,"secret":"090a0b0c"}"#)
            .expect("write share.key");

        let result = DkgOutput::load(temp_dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, DkgError::Serialization(_)));
    }

    #[test]
    fn test_dkg_output_load_invalid_share_key() {
        let temp_dir = TempDir::new().expect("create temp dir");

        std::fs::write(
            temp_dir.path().join("output.json"),
            r#"{"group_public_key":"01020304","public_polynomial":"05060708","threshold":2,"participants":3,"participant_keys":[]}"#,
        )
        .expect("write output.json");
        std::fs::write(temp_dir.path().join("share.key"), "not valid json")
            .expect("write share.key");

        let result = DkgOutput::load(temp_dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, DkgError::Serialization(_)));
    }

    #[test]
    fn test_dkg_output_load_invalid_hex_in_group_key() {
        let temp_dir = TempDir::new().expect("create temp dir");

        std::fs::write(
            temp_dir.path().join("output.json"),
            r#"{"group_public_key":"not_hex!","public_polynomial":"05060708","threshold":2,"participants":3,"participant_keys":[]}"#,
        )
        .expect("write output.json");
        std::fs::write(temp_dir.path().join("share.key"), r#"{"index":1,"secret":"090a0b0c"}"#)
            .expect("write share.key");

        let result = DkgOutput::load(temp_dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, DkgError::Serialization(_)));
    }

    #[test]
    fn test_dkg_output_load_invalid_hex_in_share_secret() {
        let temp_dir = TempDir::new().expect("create temp dir");

        std::fs::write(
            temp_dir.path().join("output.json"),
            r#"{"group_public_key":"01020304","public_polynomial":"05060708","threshold":2,"participants":3,"participant_keys":[]}"#,
        )
        .expect("write output.json");
        std::fs::write(temp_dir.path().join("share.key"), r#"{"index":1,"secret":"invalid!"}"#)
            .expect("write share.key");

        let result = DkgOutput::load(temp_dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, DkgError::Serialization(_)));
    }

    #[test]
    fn test_dkg_output_with_empty_participant_keys() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let mut output = create_test_output();
        output.participant_keys = vec![];

        output.save(temp_dir.path()).expect("save output");
        let loaded = DkgOutput::load(temp_dir.path()).expect("load output");

        assert!(loaded.participant_keys.is_empty());
    }

    #[test]
    fn test_dkg_output_clone() {
        let output = create_test_output();
        let cloned = output.clone();
        assert_eq!(output.group_public_key, cloned.group_public_key);
        assert_eq!(output.threshold, cloned.threshold);
    }

    #[test]
    fn test_dkg_output_debug() {
        let output = create_test_output();
        let debug = format!("{:?}", output);
        assert!(debug.contains("DkgOutput"));
        assert!(debug.contains("threshold"));
    }

    #[test]
    fn test_serde_json_error_conversion() {
        let result: Result<serde_json::Value, _> = serde_json::from_str("{invalid}");
        let json_err = result.unwrap_err();
        let dkg_err: DkgError = json_err.into();
        assert!(matches!(dkg_err, DkgError::Serialization(_)));
    }

    #[test]
    fn test_dkg_output_save_creates_pretty_json() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let output = create_test_output();
        output.save(temp_dir.path()).expect("save output");

        let output_content =
            std::fs::read_to_string(temp_dir.path().join("output.json")).expect("read output.json");
        assert!(output_content.contains('\n'));

        let share_content =
            std::fs::read_to_string(temp_dir.path().join("share.key")).expect("read share.key");
        assert!(share_content.contains('\n'));
    }
}
