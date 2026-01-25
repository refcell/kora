#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod adapter;
pub use adapter::StateDbAdapter;

mod context;
pub use context::BlockContext;

mod error;
pub use error::ExecutionError;

mod outcome;
pub use outcome::{ExecutionOutcome, Log, TransactionReceipt};

mod revm;
pub use revm::RevmExecutor;

mod traits;
pub use traits::BlockExecutor;
