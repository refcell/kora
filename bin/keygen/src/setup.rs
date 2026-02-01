//! Generates initial configuration for a Kora devnet.

use std::{collections::BTreeMap, fs, path::PathBuf};

use clap::Args;
use commonware_codec::Encode;
use commonware_cryptography::{Signer, ed25519};
use eyre::{Result, WrapErr};
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Args, Debug)]
pub(crate) struct SetupArgs {
    #[arg(long, default_value = "4")]
    pub validators: usize,

    #[arg(long, default_value = "3")]
    pub threshold: u32,

    #[arg(long, default_value = "1337")]
    pub chain_id: u64,

    #[arg(long, default_value = "/shared")]
    pub output_dir: PathBuf,

    #[arg(long, default_value = "30303")]
    pub base_port: u16,
}

#[derive(Serialize, Deserialize)]
struct PeersConfig {
    validators: usize,
    threshold: u32,
    participants: Vec<String>,
    bootstrappers: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize)]
struct GenesisConfig {
    chain_id: u64,
    timestamp: u64,
    allocations: Vec<GenesisAllocation>,
}

#[derive(Serialize, Deserialize)]
struct GenesisAllocation {
    address: String,
    balance: String,
}

#[derive(Serialize, Deserialize)]
struct NodeSetupConfig {
    validator_index: usize,
    public_key: String,
    port: u16,
}

pub(crate) fn run(args: SetupArgs) -> Result<()> {
    tracing::info!(
        validators = args.validators,
        threshold = args.threshold,
        chain_id = args.chain_id,
        "Generating devnet configuration"
    );

    fs::create_dir_all(&args.output_dir).wrap_err("Failed to create output directory")?;

    let mut participants = Vec::with_capacity(args.validators);
    let mut bootstrappers = BTreeMap::new();

    for i in 0..args.validators {
        let node_dir = args.output_dir.join(format!("node{}", i));
        fs::create_dir_all(&node_dir)
            .wrap_err_with(|| format!("Failed to create node{} dir", i))?;

        let key_path = node_dir.join("validator.key");
        let key = if key_path.exists() {
            tracing::info!(node = i, "Loading existing identity key");
            let bytes = fs::read(&key_path)?;
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&bytes);
            ed25519::PrivateKey::from(ed25519_consensus::SigningKey::from(seed))
        } else {
            tracing::info!(node = i, "Generating new identity key");
            let mut seed = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut seed);
            fs::write(&key_path, seed)?;
            ed25519::PrivateKey::from(ed25519_consensus::SigningKey::from(seed))
        };

        let public_key = key.public_key();
        let pk_hex = hex::encode(Encode::encode(&public_key));

        participants.push(pk_hex.clone());

        let hostname = format!("node{}:{}", i, args.base_port);
        bootstrappers.insert(pk_hex.clone(), hostname);

        let node_config =
            NodeSetupConfig { validator_index: i, public_key: pk_hex, port: args.base_port };
        let config_path = node_dir.join("setup.json");
        fs::write(&config_path, serde_json::to_string_pretty(&node_config)?)?;

        tracing::info!(node = i, path = ?key_path, "Wrote identity key");
    }

    let peers = PeersConfig {
        validators: args.validators,
        threshold: args.threshold,
        participants,
        bootstrappers,
    };
    let peers_path = args.output_dir.join("peers.json");
    fs::write(&peers_path, serde_json::to_string_pretty(&peers)?)?;
    tracing::info!(path = ?peers_path, "Wrote peers configuration");

    let genesis = GenesisConfig {
        chain_id: args.chain_id,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        allocations: vec![GenesisAllocation {
            address: "0x0000000000000000000000000000000000000001".to_string(),
            balance: "1000000000000000000000000".to_string(),
        }],
    };
    let genesis_path = args.output_dir.join("genesis.json");
    fs::write(&genesis_path, serde_json::to_string_pretty(&genesis)?)?;
    tracing::info!(path = ?genesis_path, "Wrote genesis configuration");

    tracing::info!("Setup complete");
    tracing::info!("  Validators: {}", args.validators);
    tracing::info!("  Threshold:  {}", args.threshold);
    tracing::info!("  Chain ID:   {}", args.chain_id);

    Ok(())
}
