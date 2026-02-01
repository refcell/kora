#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use kora_config as _;

mod application;
pub use application::{ConsensusApplication, ConsensusApplicationExt};

mod error;
pub use error::ConsensusError;

mod traits;
pub use traits::{Digest, Mempool, SeedTracker, Snapshot, SnapshotStore, TxId};

mod ledger;
pub use ledger::LedgerView;

mod proposal;
pub use proposal::ProposalBuilder;

mod execution;
pub use execution::BlockExecution;

pub mod components;
