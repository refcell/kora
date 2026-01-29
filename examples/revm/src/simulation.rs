//! Simulation harness for the example chain.
//!
//! Spawns N nodes in a single process using the tokio runtime and the simulated P2P transport.
//! The harness waits for a fixed number of finalized blocks and asserts all nodes converge on the
//! same head, state commitment, and balances.

use std::time::Duration;

use alloy_evm::revm::primitives::B256;
use anyhow::Context as _;
use commonware_cryptography::ed25519;
use commonware_p2p::{Manager as _, simulated};
use commonware_runtime::{Metrics as _, Runner as _, tokio};
use commonware_utils::{TryCollect as _, ordered::Set};
use futures::{StreamExt as _, channel::mpsc};
use kora_domain::{BootstrapConfig, ConsensusDigest, FinalizationEvent, StateRoot};
use kora_sys::FileLimitHandler;

use kora_transport_sim::SimContext;

use crate::{
    application::{ThresholdScheme, start_node, threshold_schemes},
    config::SimConfig,
    environment::{SimEnvironment, SimTransport},
    outcome::SimOutcome,
};

/// Maximum size (bytes) of a single simulated network message.
pub(super) const MAX_MSG_SIZE: usize = 1024 * 1024;
/// Fixed latency (milliseconds) for simulated P2P links.
pub(super) const P2P_LINK_LATENCY_MS: u64 = 5;

type NodeHandle = crate::application::NodeHandle;
/// Run the multi-node simulation and return the final outcome.
pub(crate) fn simulate(cfg: SimConfig) -> anyhow::Result<SimOutcome> {
    FileLimitHandler::new().raise();
    // Tokio runtime required for WrapDatabaseAsync in the QMDB adapter.
    let executor = tokio::Runner::default();
    executor.start(|context| async move { run_sim(context, cfg).await })
}

async fn run_sim(context: tokio::Context, cfg: SimConfig) -> anyhow::Result<SimOutcome> {
    let (participants_vec, schemes) = threshold_schemes(cfg.seed, cfg.nodes)?;
    let participants_set = participants_set(&participants_vec)?;

    let mut transport = start_network(&context, participants_set).await;
    connect_all_peers(&mut transport, &participants_vec).await?;

    let demo = crate::demo::DemoTransfer::new();
    let bootstrap = BootstrapConfig::new(demo.alloc.clone(), vec![demo.tx.clone()]);

    let (nodes, mut finalized_rx) =
        start_all_nodes(&context, &mut transport, &participants_vec, &schemes, &bootstrap).await?;

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

/// Spawn all nodes (application + consensus) for a simulation run.
async fn start_all_nodes(
    context: &tokio::Context,
    transport: &mut SimTransport,
    participants: &[ed25519::PublicKey],
    schemes: &[ThresholdScheme],
    bootstrap: &BootstrapConfig,
) -> anyhow::Result<(Vec<NodeHandle>, mpsc::UnboundedReceiver<FinalizationEvent>)> {
    let (finalized_tx, finalized_rx) = mpsc::unbounded::<FinalizationEvent>();
    let mut nodes = Vec::with_capacity(participants.len());
    let mut env = SimEnvironment::new(context.clone(), transport);

    for (i, pk) in participants.iter().cloned().enumerate() {
        let handle =
            start_node(&mut env, i, pk, schemes[i].clone(), finalized_tx.clone(), bootstrap)
                .await?;
        nodes.push(handle);
    }

    Ok((nodes, finalized_rx))
}

/// Ensure the provided round-robin identity list is unique for the simulation.
fn participants_set(
    participants: &[ed25519::PublicKey],
) -> anyhow::Result<Set<ed25519::PublicKey>> {
    participants
        .iter()
        .cloned()
        .try_collect()
        .map_err(|_| anyhow::anyhow!("participant public keys are not unique"))
}

/// Boot the simulated p2p network and register the participant set.
async fn start_network(
    context: &tokio::Context,
    participants: Set<ed25519::PublicKey>,
) -> SimTransport {
    let (network, transport) = simulated::Network::new(
        SimContext::new(context.with_label("network")),
        simulated::Config {
            max_size: MAX_MSG_SIZE as u32,
            disconnect_on_block: true,
            tracked_peer_sets: None,
        },
    );
    network.start();

    transport.manager().update(0, participants).await;
    transport
}

/// Connect all peers in a full mesh with fixed links.
async fn connect_all_peers(
    transport: &mut SimTransport,
    peers: &[ed25519::PublicKey],
) -> anyhow::Result<()> {
    for a in peers.iter() {
        for b in peers.iter() {
            if a == b {
                continue;
            }
            transport
                .add_link(
                    a.clone(),
                    b.clone(),
                    simulated::Link {
                        latency: Duration::from_millis(P2P_LINK_LATENCY_MS),
                        jitter: Duration::from_millis(0),
                        success_rate: 1.0,
                    },
                )
                .await
                .context("add_link")?;
        }
    }
    Ok(())
}

/// Wait until each node has observed `blocks` finalizations and return the common head digest.
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

/// Query each node's application store at `head` and assert they all agree on the outcome.
async fn assert_all_nodes_converged(
    nodes: &[NodeHandle],
    head: ConsensusDigest,
    demo: &crate::demo::DemoTransfer,
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
    use alloy_evm::revm::primitives::U256;

    use super::*;

    #[test]
    fn test_sim_smoke() {
        // Tokio runtime required for WrapDatabaseAsync in the QMDB adapter.
        let outcome = simulate(SimConfig { nodes: 4, blocks: 3, seed: 42 }).unwrap();
        assert_eq!(outcome.from_balance, U256::from(1_000_000u64 - 100));
        assert_eq!(outcome.to_balance, U256::from(100u64));
    }
}
