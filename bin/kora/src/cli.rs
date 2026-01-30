use std::path::PathBuf;

use clap::{Parser, Subcommand};
use kora_config::NodeConfig;
use kora_domain::BootstrapConfig;
use kora_rpc::NodeState;
use kora_runner::{ProductionRunner, load_threshold_scheme};
use kora_service::LegacyNodeService;

#[derive(Parser, Debug)]
#[command(name = "kora")]
#[command(about = "A minimal commonware + revm execution client")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(short, long, value_name = "FILE", global = true)]
    pub config: Option<PathBuf>,

    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[arg(long, global = true)]
    pub chain_id: Option<u64>,

    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    Dkg(DkgArgs),
    Validator(ValidatorArgs),
}

#[derive(clap::Args, Debug)]
pub(crate) struct DkgArgs {
    /// Path to peers.json file containing participant information.
    #[arg(long)]
    pub peers: PathBuf,

    /// Force restart the DKG ceremony, ignoring any persisted state.
    #[arg(long, default_value = "false")]
    pub force_restart: bool,
}

#[derive(clap::Args, Debug)]
pub(crate) struct ValidatorArgs {
    #[arg(long)]
    pub peers: Option<PathBuf>,
}

impl Cli {
    pub(crate) fn load_config(&self) -> eyre::Result<NodeConfig> {
        let mut config = NodeConfig::load(self.config.as_deref())?;

        if let Some(chain_id) = self.chain_id {
            config.chain_id = chain_id;
        }
        if let Some(ref data_dir) = self.data_dir {
            config.data_dir = data_dir.clone();
        }

        Ok(config)
    }

    pub(crate) fn run(self) -> eyre::Result<()> {
        match &self.command {
            Some(Commands::Dkg(args)) => self.run_dkg(args),
            Some(Commands::Validator(args)) => self.run_validator(args),
            None => self.run_legacy(),
        }
    }

    fn run_dkg(&self, args: &DkgArgs) -> eyre::Result<()> {
        use kora_dkg::{DkgCeremony, DkgConfig};

        let node_config = self.load_config()?;
        tracing::info!(chain_id = node_config.chain_id, "Starting DKG ceremony");

        let peers = load_peers(&args.peers)?;
        let identity_key = node_config.validator_key()?;
        let my_pk = commonware_cryptography::Signer::public_key(&identity_key);

        let validator_index = peers
            .participants
            .iter()
            .position(|pk| *pk == my_pk)
            .ok_or_else(|| eyre::eyre!("Our public key not found in participants list"))?;

        let dkg_config = DkgConfig {
            identity_key,
            validator_index,
            participants: peers.participants,
            threshold: peers.threshold,
            chain_id: node_config.chain_id,
            data_dir: node_config.data_dir.clone(),
            listen_addr: node_config.network.listen_addr.parse()?,
            bootstrap_peers: peers.bootstrappers,
            timeout: std::time::Duration::from_secs(300),
        };

        let ceremony = if args.force_restart {
            DkgCeremony::new_with_force_restart(dkg_config, true)
        } else {
            DkgCeremony::new(dkg_config)
        };

        let rt = tokio::runtime::Runtime::new()?;
        let output = rt.block_on(ceremony.run())?;

        tracing::info!(share_index = output.share_index, "DKG ceremony completed successfully");

        Ok(())
    }

    fn run_validator(&self, args: &ValidatorArgs) -> eyre::Result<()> {
        let mut config = self.load_config()?;

        tracing::info!(chain_id = config.chain_id, "Starting validator");

        if !kora_dkg::DkgOutput::exists(&config.data_dir) {
            return Err(eyre::eyre!(
                "DKG output not found. Run 'kora dkg' first to generate threshold shares."
            ));
        }

        let dkg_output = kora_dkg::DkgOutput::load(&config.data_dir)?;
        tracing::info!(share_index = dkg_output.share_index, "Loaded DKG output");

        let scheme = load_threshold_scheme(&config.data_dir)
            .map_err(|e| eyre::eyre!("Failed to load threshold scheme: {}", e))?;
        tracing::info!("Loaded threshold signing scheme");

        if let Some(ref peers_path) = args.peers {
            let peers = load_peers(peers_path)?;
            config.network.bootstrap_peers = peers
                .bootstrappers
                .iter()
                .map(|(pk, addr)| format!("{}@{}", hex::encode(pk.as_ref()), addr))
                .collect();
            tracing::info!(
                bootstrap_peers = config.network.bootstrap_peers.len(),
                "Loaded bootstrap peers from peers.json"
            );
        }

        let genesis_path = config.data_dir.join("genesis.json");
        let bootstrap = BootstrapConfig::load(&genesis_path)
            .map_err(|e| eyre::eyre!("Failed to load genesis: {}", e))?;
        tracing::info!(allocations = bootstrap.genesis_alloc.len(), "Loaded genesis configuration");

        const GAS_LIMIT: u64 = 30_000_000;

        // Create RPC state that will be updated by consensus
        let rpc_port = 8545 + dkg_output.share_index as u16;
        let rpc_addr: std::net::SocketAddr = format!("0.0.0.0:{}", rpc_port).parse()?;
        let node_state = NodeState::new(config.chain_id, dkg_output.share_index);

        let runner = ProductionRunner::new(scheme, config.chain_id, GAS_LIMIT, bootstrap)
            .with_rpc(node_state, rpc_addr);

        runner.run_standalone(config).map_err(|e| eyre::eyre!("Runner failed: {}", e.0))
    }

    fn run_legacy(&self) -> eyre::Result<()> {
        let config = self.load_config()?;

        tracing::info!(chain_id = config.chain_id, "Starting node (legacy mode)");
        tracing::debug!(?config, "Full configuration");

        LegacyNodeService::new(config).run()
    }
}

#[derive(Debug)]
struct PeersInfo {
    participants: Vec<commonware_cryptography::ed25519::PublicKey>,
    threshold: u32,
    bootstrappers: Vec<(commonware_cryptography::ed25519::PublicKey, String)>,
}

fn load_peers(path: &PathBuf) -> eyre::Result<PeersInfo> {
    use commonware_codec::ReadExt;

    let content = std::fs::read_to_string(path)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;

    let threshold =
        json["threshold"].as_u64().ok_or_else(|| eyre::eyre!("missing threshold"))? as u32;

    let participants_hex: Vec<String> = json["participants"]
        .as_array()
        .ok_or_else(|| eyre::eyre!("missing participants"))?
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    let mut participants = Vec::with_capacity(participants_hex.len());
    for pk_hex in &participants_hex {
        let bytes = hex::decode(pk_hex)?;
        let pk = commonware_cryptography::ed25519::PublicKey::read(&mut bytes.as_slice())?;
        participants.push(pk);
    }

    let bootstrappers_obj =
        json["bootstrappers"].as_object().ok_or_else(|| eyre::eyre!("missing bootstrappers"))?;

    let mut bootstrappers = Vec::new();
    for (pk_hex, addr) in bootstrappers_obj {
        let bytes = hex::decode(pk_hex)?;
        let pk = commonware_cryptography::ed25519::PublicKey::read(&mut bytes.as_slice())?;
        let addr_str = addr.as_str().ok_or_else(|| eyre::eyre!("invalid address"))?;
        bootstrappers.push((pk, addr_str.to_string()));
    }

    Ok(PeersInfo { participants, threshold, bootstrappers })
}
