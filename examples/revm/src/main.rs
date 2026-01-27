//! REVM-based example chain driven by threshold-simplex.
//!
//! This example uses `alloy-evm` as the integration layer above `revm` and keeps the execution
//! backend generic over the database trait boundary (`Database` + `DatabaseCommit`).

use clap::{Arg, Command, value_parser};

pub mod application;
pub use application::execution::{
    CHAIN_ID, ExecutionOutcome, SEED_PRECOMPILE_ADDRESS_BYTES, evm_env, execute_txs,
    seed_precompile_address,
};

pub type ConsensusDigest = commonware_cryptography::sha256::Digest;
pub type PublicKey = commonware_cryptography::ed25519::PublicKey;
pub(crate) type FinalizationEvent = (u32, ConsensusDigest);

pub mod domain;
pub use domain::{
    AccountChange, Block, BlockCfg, BlockId, BootstrapConfig, StateChanges, StateChangesCfg,
    StateRoot, Tx, TxCfg, TxId, block_id,
};

pub mod qmdb;

pub mod simulation;
pub use simulation::{SimConfig, SimOutcome, simulate};

fn main() -> anyhow::Result<()> {
    let matches = Command::new("kora-revm-example")
        .about("threshold-simplex + EVM execution example")
        .arg(
            Arg::new("nodes")
                .long("nodes")
                .required(false)
                .default_value("4")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("blocks")
                .long("blocks")
                .required(false)
                .default_value("3")
                .value_parser(value_parser!(u64)),
        )
        .arg(
            Arg::new("seed")
                .long("seed")
                .required(false)
                .default_value("1")
                .value_parser(value_parser!(u64)),
        )
        .get_matches();

    let nodes = *matches.get_one::<usize>("nodes").expect("defaulted");
    let blocks = *matches.get_one::<u64>("blocks").expect("defaulted");
    let seed = *matches.get_one::<u64>("seed").expect("defaulted");

    let outcome = simulate(SimConfig { nodes, blocks, seed })?;
    println!("finalized head: {:?}", outcome.head);
    println!("state root: {:?}", outcome.state_root);
    println!("seed: {:?}", outcome.seed);
    println!("from balance: {:?}", outcome.from_balance);
    println!("to balance: {:?}", outcome.to_balance);
    Ok(())
}
