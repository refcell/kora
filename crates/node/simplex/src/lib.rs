//! Commonware simplex consensus engine integration for Kora.
#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod config;
pub use config::{
    DEFAULT_ACTIVITY_TIMEOUT, DEFAULT_FETCH_CONCURRENT, DEFAULT_FETCH_TIMEOUT,
    DEFAULT_LEADER_TIMEOUT, DEFAULT_MAILBOX_SIZE, DEFAULT_NOTARIZATION_TIMEOUT,
    DEFAULT_NULLIFY_RETRY, DEFAULT_REPLAY_BUFFER, DEFAULT_SKIP_TIMEOUT, DEFAULT_WRITE_BUFFER,
    DefaultConfig,
};

mod engine;
pub use engine::DefaultEngine;

mod pool;
pub use pool::{DEFAULT_PAGE_SIZE, DEFAULT_POOL_CAPACITY, DefaultPool};

mod quota;
pub use quota::{DEFAULT_REQUESTS_PER_SECOND, DefaultQuota};
