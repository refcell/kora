use std::sync::{Arc, Mutex};

use alloy_primitives::{Address, B256, U256};
use anyhow::Context as _;
use commonware_cryptography::ed25519;
use commonware_p2p::{Manager as _, simulated};
use commonware_runtime::{Metrics as _, Runner as _, tokio};
use commonware_utils::{TryCollect as _, ordered::Set};
use futures::{StreamExt as _, channel::mpsc};
use k256::ecdsa::SigningKey;
use kora_config::NodeConfig;
use kora_crypto::{ThresholdScheme, threshold_schemes};
use kora_domain::{BootstrapConfig, ConsensusDigest, FinalizationEvent, StateRoot, Tx, evm::Evm};
use kora_service::KoraNodeService;
use kora_sys::FileLimitHandler;
use kora_transport_sim::{SimContext, SimControl, SimLinkConfig, SimTransportProvider};

use crate::{
    app::CHAIN_ID, cli::SimConfig, handle::NodeHandle, outcome::SimOutcome, runner::RevmNodeRunner,
};

const MAX_MSG_SIZE: usize = 1024 * 1024;

struct DemoTransfer {
    from: Address,
    to: Address,
    alloc: Vec<(Address, U256)>,
    tx: Tx,
    expected_from: U256,
    expected_to: U256,
}

impl DemoTransfer {
    fn new() -> Self {
        let sender = SigningKey::from_bytes(&[1u8; 32].into()).expect("valid sender key");
        let receiver = SigningKey::from_bytes(&[2u8; 32].into()).expect("valid receiver key");
        let from = Evm::address_from_key(&sender);
        let to = Evm::address_from_key(&receiver);
        let tx = Evm::sign_eip1559_transfer(&sender, CHAIN_ID, to, U256::from(100u64), 0, 21_000);

        Self {
            from,
            to,
            alloc: vec![(from, U256::from(1_000_000u64)), (to, U256::ZERO)],
            tx,
            expected_from: U256::from(1_000_000u64 - 100),
            expected_to: U256::from(100u64),
        }
    }
}

pub(crate) fn simulate(cfg: SimConfig) -> anyhow::Result<SimOutcome> {
    FileLimitHandler::new().raise();
    let executor = tokio::Runner::default();
    executor.start(|context| async move { run_sim(context, cfg).await })
}

async fn run_sim(context: tokio::Context, cfg: SimConfig) -> anyhow::Result<SimOutcome> {
    let (participants_vec, schemes) = threshold_schemes(cfg.seed, cfg.nodes)?;
    let participants_set = participants_set(&participants_vec)?;

    let mut sim_control = start_network(&context, participants_set).await;
    sim_control
        .connect_all(&participants_vec, SimLinkConfig::default())
        .await
        .context("connect_all")?;
    let sim_control = Arc::new(Mutex::new(sim_control));

    let demo = DemoTransfer::new();
    let bootstrap = BootstrapConfig::new(demo.alloc.clone(), vec![demo.tx.clone()]);

    let (nodes, mut finalized_rx) =
        start_all_nodes(&context, &sim_control, &participants_vec, &schemes, &bootstrap).await?;

    let head = wait_for_finalized_head(&mut finalized_rx, cfg.nodes, cfg.blocks).await?;
    let (state_root, seed) = assert_all_nodes_converged(&nodes, head, &demo).await?;

    Ok(SimOutcome {
        head,
        state_root,
        seed,
        from_balance: demo.expected_from,
        to_balance: demo.expected_to,
    })
}

async fn start_all_nodes(
    context: &tokio::Context,
    sim_control: &Arc<Mutex<SimControl<ed25519::PublicKey>>>,
    participants: &[ed25519::PublicKey],
    schemes: &[ThresholdScheme],
    bootstrap: &BootstrapConfig,
) -> anyhow::Result<(Vec<NodeHandle>, mpsc::UnboundedReceiver<FinalizationEvent>)> {
    let (finalized_tx, finalized_rx) = mpsc::unbounded::<FinalizationEvent>();
    let mut nodes = Vec::with_capacity(participants.len());

    let manager = {
        let control = sim_control.lock().map_err(|_| anyhow::anyhow!("lock poisoned"))?;
        control.manager()
    };

    for (i, pk) in participants.iter().cloned().enumerate() {
        let runner = RevmNodeRunner {
            index: i,
            public_key: pk.clone(),
            scheme: schemes[i].clone(),
            bootstrap: bootstrap.clone(),
            finalized_tx: finalized_tx.clone(),
            manager: manager.clone(),
        };

        let transport_provider = SimTransportProvider::new(Arc::clone(sim_control), pk.clone());
        let node_config = NodeConfig::default();
        let service = KoraNodeService::new(runner, transport_provider, node_config);
        let handle = service
            .run_with_context(context.clone())
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        nodes.push(handle);
    }

    Ok((nodes, finalized_rx))
}

fn participants_set(
    participants: &[ed25519::PublicKey],
) -> anyhow::Result<Set<ed25519::PublicKey>> {
    participants
        .iter()
        .cloned()
        .try_collect()
        .map_err(|_| anyhow::anyhow!("participant public keys are not unique"))
}

async fn start_network(
    context: &tokio::Context,
    participants: Set<ed25519::PublicKey>,
) -> SimControl<ed25519::PublicKey> {
    let (network, oracle) = simulated::Network::new(
        SimContext::new(context.with_label("network")),
        simulated::Config {
            max_size: MAX_MSG_SIZE as u32,
            disconnect_on_block: true,
            tracked_peer_sets: None,
        },
    );
    network.start();

    let control = SimControl::new(oracle);
    control.manager().update(0, participants).await;
    control
}

async fn wait_for_finalized_head(
    finalized_rx: &mut mpsc::UnboundedReceiver<FinalizationEvent>,
    nodes: usize,
    blocks: u64,
) -> anyhow::Result<ConsensusDigest> {
    if blocks == 0 {
        return Err(anyhow::anyhow!("blocks must be greater than zero"));
    }

    let mut counts = vec![0u64; nodes];
    let mut nth = vec![None; nodes];
    while nth.iter().any(Option::is_none) {
        let Some((node, digest)) = finalized_rx.next().await else {
            break;
        };
        let idx = node as usize;
        if nth[idx].is_some() {
            continue;
        }
        counts[idx] += 1;
        if counts[idx] == blocks {
            nth[idx] = Some(digest);
        }
    }

    let head =
        nth.first().and_then(|d| *d).ok_or_else(|| anyhow::anyhow!("missing finalization"))?;
    for (i, d) in nth.iter().enumerate() {
        let Some(d) = d else {
            return Err(anyhow::anyhow!("node {i} missing finalization"));
        };
        if *d != head {
            return Err(anyhow::anyhow!("divergent finalized heads"));
        }
    }
    Ok(head)
}

async fn assert_all_nodes_converged(
    nodes: &[NodeHandle],
    head: ConsensusDigest,
    demo: &DemoTransfer,
) -> anyhow::Result<(StateRoot, B256)> {
    let mut state_root = None;
    let mut seed = None;
    for node in nodes.iter() {
        let from_balance = node
            .query_balance(head, demo.from)
            .await
            .ok_or_else(|| anyhow::anyhow!("missing from balance"))?;
        let to_balance = node
            .query_balance(head, demo.to)
            .await
            .ok_or_else(|| anyhow::anyhow!("missing to balance"))?;
        if from_balance != demo.expected_from || to_balance != demo.expected_to {
            return Err(anyhow::anyhow!("unexpected balances"));
        }

        let root = node
            .query_state_root(head)
            .await
            .ok_or_else(|| anyhow::anyhow!("missing state root"))?;
        state_root = match state_root {
            None => Some(root),
            Some(prev) if prev == root => Some(prev),
            Some(_) => return Err(anyhow::anyhow!("divergent state roots")),
        };

        let node_seed =
            node.query_seed(head).await.ok_or_else(|| anyhow::anyhow!("missing seed"))?;
        seed = match seed {
            None => Some(node_seed),
            Some(prev) if prev == node_seed => Some(prev),
            Some(_) => return Err(anyhow::anyhow!("divergent seeds")),
        };
    }

    let state_root = state_root.ok_or_else(|| anyhow::anyhow!("missing state root"))?;
    let seed = seed.ok_or_else(|| anyhow::anyhow!("missing seed"))?;
    Ok((state_root, seed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sim_smoke() {
        let outcome = simulate(SimConfig { nodes: 4, blocks: 3, seed: 42 }).unwrap();
        assert_eq!(outcome.from_balance, U256::from(1_000_000u64 - 100));
        assert_eq!(outcome.to_balance, U256::from(100u64));
    }
}
