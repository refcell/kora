#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod block;
pub use block::KoraBlock;

mod error;
pub use error::ConsensusError;

mod traits;
// Re-export executor types
pub use kora_executor::{BlockExecutor, ExecutionOutcome};
pub use traits::{Digest, Mempool, SeedTracker, Snapshot, SnapshotStore, TxId};

pub mod components;
