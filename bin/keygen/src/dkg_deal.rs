//! Trusted dealer DKG for devnet.
//!
//! Generates all BLS12-381 threshold shares using a single trusted dealer.
//! This is NOT secure for production but allows testing the validator workflow.

use std::{fs, path::PathBuf};

use clap::Args;
use commonware_codec::{ReadExt, Write as _};
use commonware_cryptography::bls12381::{
    dkg,
    primitives::{sharing::Mode, variant::MinSig},
};
use commonware_utils::{N3f1, TryCollect, ordered::Set};
use eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};

#[derive(Args, Debug)]
pub(crate) struct DkgDealArgs {
    #[arg(long, default_value = "4")]
    pub validators: usize,

    #[arg(long, default_value = "3")]
    pub threshold: u32,

    #[arg(long, default_value = "/shared")]
    pub output_dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct OutputJson {
    group_public_key: String,
    public_polynomial: String,
    threshold: u32,
    participants: usize,
    participant_keys: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct ShareJson {
    index: u32,
    secret: String,
}

pub(crate) fn run(args: DkgDealArgs) -> Result<()> {
    tracing::info!(
        validators = args.validators,
        threshold = args.threshold,
        "Running trusted dealer DKG"
    );

    let mut participants = Vec::with_capacity(args.validators);
    for i in 0..args.validators {
        let node_dir = args.output_dir.join(format!("node{}", i));
        let setup_path = node_dir.join("setup.json");

        let setup_str = fs::read_to_string(&setup_path)
            .wrap_err_with(|| format!("Failed to read setup.json for node{}", i))?;
        let setup: serde_json::Value = serde_json::from_str(&setup_str)?;

        let pk_hex = setup["public_key"]
            .as_str()
            .ok_or_else(|| eyre::eyre!("missing public_key in setup.json"))?;

        let pk_bytes = hex::decode(pk_hex)?;
        let pk = commonware_cryptography::ed25519::PublicKey::read(&mut pk_bytes.as_slice())
            .map_err(|e| eyre::eyre!("Failed to decode public key: {:?}", e))?;

        participants.push(pk);
        tracing::debug!(node = i, pk = %pk_hex, "Loaded participant");
    }

    let participants_set: Set<commonware_cryptography::ed25519::PublicKey> = participants
        .iter()
        .cloned()
        .try_collect()
        .map_err(|_| eyre::eyre!("Duplicate participants"))?;

    let mut rng = rand::rngs::OsRng;

    tracing::info!("Generating BLS threshold key shares");
    let (public_output, shares) =
        dkg::deal::<MinSig, _, N3f1>(&mut rng, Mode::default(), participants_set)
            .map_err(|e| eyre::eyre!("DKG deal failed: {:?}", e))?;

    let sharing = public_output.public();

    let mut public_polynomial_bytes = Vec::new();
    sharing.write(&mut public_polynomial_bytes);

    let group_key = sharing.public();
    let mut group_key_bytes = Vec::new();
    group_key.write(&mut group_key_bytes);

    tracing::info!(
        group_key = hex::encode(&group_key_bytes),
        polynomial_len = public_polynomial_bytes.len(),
        "Generated group public key and polynomial"
    );

    let participant_keys: Vec<String> = participants
        .iter()
        .map(|pk| {
            let mut bytes = Vec::new();
            pk.write(&mut bytes);
            hex::encode(bytes)
        })
        .collect();

    for (i, pk) in participants.iter().enumerate() {
        let share =
            shares.get_value(pk).ok_or_else(|| eyre::eyre!("Missing share for node{}", i))?;

        let mut share_bytes = Vec::new();
        share.write(&mut share_bytes);

        let node_dir = args.output_dir.join(format!("node{}", i));

        let output_json = OutputJson {
            group_public_key: hex::encode(&group_key_bytes),
            public_polynomial: hex::encode(&public_polynomial_bytes),
            threshold: args.threshold,
            participants: args.validators,
            participant_keys: participant_keys.clone(),
        };
        let output_path = node_dir.join("output.json");
        fs::write(&output_path, serde_json::to_string_pretty(&output_json)?)?;

        let share_json = ShareJson { index: i as u32, secret: hex::encode(&share_bytes) };
        let share_path = node_dir.join("share.key");
        fs::write(&share_path, serde_json::to_string_pretty(&share_json)?)?;

        tracing::info!(node = i, "Wrote DKG output and share");
    }

    tracing::info!("Trusted dealer DKG complete");
    tracing::info!("  Validators: {}", args.validators);
    tracing::info!("  Threshold:  {}", args.threshold);

    Ok(())
}
