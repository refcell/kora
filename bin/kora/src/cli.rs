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
    Secondary(SecondaryArgs),
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

#[derive(clap::Args, Debug)]
pub(crate) struct SecondaryArgs {
    /// Path to peers.json file containing primary and secondary peer information.
    #[arg(long)]
    pub peers: PathBuf,
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
            Some(Commands::Secondary(args)) => self.run_secondary(args),
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

        let mut secondary_participants = Vec::new();
        if let Some(ref peers_path) = args.peers {
            let peers = load_peers(peers_path)?;
            config.network.bootstrap_peers = format_bootstrappers(&peers.bootstrappers);
            tracing::info!(
                bootstrap_peers = config.network.bootstrap_peers.len(),
                "Loaded bootstrap peers from peers.json"
            );

            secondary_participants = peers.secondary_participants;
        }

        let genesis_path = config.data_dir.join("genesis.json");
        let bootstrap = BootstrapConfig::load(&genesis_path)
            .map_err(|e| eyre::eyre!("Failed to load genesis: {}", e))?;
        tracing::info!(allocations = bootstrap.genesis_alloc.len(), "Loaded genesis configuration");

        let rpc_addr: std::net::SocketAddr = "0.0.0.0:8545".parse()?;
        let node_state = NodeState::new(config.chain_id, dkg_output.share_index);

        let runner = ProductionRunner::new(
            scheme,
            config.chain_id,
            kora_config::DEFAULT_GAS_LIMIT,
            bootstrap,
        )
        .with_rpc(node_state, rpc_addr)
        .with_secondary_peers(secondary_participants);

        runner.run_standalone(config).map_err(|e| eyre::eyre!("Runner failed: {}", e.0))
    }

    fn run_secondary(&self, args: &SecondaryArgs) -> eyre::Result<()> {
        use commonware_p2p::{Manager, TrackedPeers};
        use commonware_runtime::Runner;
        use commonware_utils::ordered::Set;
        use kora_transport::NetworkConfigExt;

        let mut config = self.load_config()?;
        let peers = load_peers(&args.peers)?;
        config.network.bootstrap_peers = format_bootstrappers(&peers.bootstrappers);

        let identity_key = config.validator_key()?;
        let my_pk = commonware_cryptography::Signer::public_key(&identity_key);
        if !peers.secondary_participants.contains(&my_pk) {
            return Err(eyre::eyre!(
                "secondary identity is not listed in peers.json secondary_participants"
            ));
        }

        tracing::info!(
            chain_id = config.chain_id,
            bootstrap_peers = config.network.bootstrap_peers.len(),
            secondary_peers = peers.secondary_participants.len(),
            "Starting secondary peer"
        );

        let executor = commonware_runtime::tokio::Runner::default();
        executor.start(|context| async move {
            let mut transport = config
                .network
                .build_local_transport(identity_key, context.clone())
                .map_err(|e| eyre::eyre!("failed to build transport: {}", e))?;

            transport
                .oracle
                .track(
                    0,
                    TrackedPeers::new(
                        Set::from_iter_dedup(peers.participants),
                        Set::from_iter_dedup(peers.secondary_participants),
                    ),
                )
                .await;

            tracing::info!("secondary peer joined network");
            futures::future::pending::<()>().await;
            #[allow(unreachable_code)]
            Ok::<(), eyre::Error>(())
        })
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
    secondary_participants: Vec<commonware_cryptography::ed25519::PublicKey>,
    threshold: u32,
    bootstrappers: Vec<(commonware_cryptography::ed25519::PublicKey, String)>,
}

fn format_bootstrappers(
    bootstrappers: &[(commonware_cryptography::ed25519::PublicKey, String)],
) -> Vec<String> {
    bootstrappers
        .iter()
        .map(|(pk, addr)| format!("{}@{}", hex::encode(pk.as_ref()), addr))
        .collect()
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

    let participants = parse_public_keys(&participants_hex)?;

    let secondary_participants_hex: Vec<String> = json["secondary_participants"]
        .as_array()
        .map(|participants| {
            participants.iter().filter_map(|v| v.as_str().map(String::from)).collect()
        })
        .unwrap_or_default();
    let secondary_participants = parse_public_keys(&secondary_participants_hex)?;

    let bootstrappers_obj =
        json["bootstrappers"].as_object().ok_or_else(|| eyre::eyre!("missing bootstrappers"))?;

    let mut bootstrappers = Vec::new();
    for (pk_hex, addr) in bootstrappers_obj {
        let bytes = hex::decode(pk_hex)?;
        let pk = commonware_cryptography::ed25519::PublicKey::read(&mut bytes.as_slice())?;
        let addr_str = addr.as_str().ok_or_else(|| eyre::eyre!("invalid address"))?;
        bootstrappers.push((pk, addr_str.to_string()));
    }

    Ok(PeersInfo { participants, secondary_participants, threshold, bootstrappers })
}

fn parse_public_keys(
    keys: &[String],
) -> eyre::Result<Vec<commonware_cryptography::ed25519::PublicKey>> {
    use commonware_codec::ReadExt;

    let mut public_keys = Vec::with_capacity(keys.len());
    for pk_hex in keys {
        let bytes = hex::decode(pk_hex)?;
        let pk = commonware_cryptography::ed25519::PublicKey::read(&mut bytes.as_slice())?;
        public_keys.push(pk);
    }

    Ok(public_keys)
}
