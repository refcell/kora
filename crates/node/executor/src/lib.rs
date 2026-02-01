//! Block execution abstractions and REVM-based implementation for Kora.

#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod adapter;
pub use adapter::StateDbAdapter;

mod config;
pub use config::{BaseFeeParams, ExecutionConfig, GasLimitBounds};

mod context;
pub use context::{BlockContext, ParentBlock};

mod error;
pub use error::ExecutionError;

mod outcome;
pub use outcome::{ExecutionOutcome, ExecutionReceipt};

mod revm;
pub use revm::{RevmExecutor, calculate_base_fee};

mod traits;
pub use traits::BlockExecutor;

mod validation;
pub use validation::{
    ACCESS_LIST_ADDRESS_GAS, ACCESS_LIST_STORAGE_KEY_GAS, MAX_BLOBS_PER_TX, TX_BASE_GAS,
    TX_CREATE_GAS, TX_DATA_NON_ZERO_GAS, TX_DATA_ZERO_GAS, TxValidator, ValidatedTx,
};
