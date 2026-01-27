//! REVM-based example chain driven by threshold-simplex.
//!
//! This example uses `alloy-evm` as the integration layer above `revm` and keeps the execution
//! backend generic over the database trait boundary (`Database` + `DatabaseCommit`).

mod application;
mod domain;
mod qmdb;
mod simulation;

pub use application::execution::{
    CHAIN_ID, ExecutionOutcome, SEED_PRECOMPILE_ADDRESS_BYTES, evm_env, execute_txs,
    seed_precompile_address,
};
pub use domain::{
    AccountChange, Block, BlockCfg, BlockId, BootstrapConfig, StateChanges, StateChangesCfg,
    StateRoot, Tx, TxCfg, TxId, block_id,
};
pub use simulation::{SimConfig, SimOutcome, simulate};

pub type ConsensusDigest = commonware_cryptography::sha256::Digest;
pub type PublicKey = commonware_cryptography::ed25519::PublicKey;
pub(crate) type FinalizationEvent = (u32, ConsensusDigest);
