//! End-to-end testing framework for Kora consensus network.

#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod harness;
pub use harness::{HarnessError, TestHarness, TestOutcome};

mod node;
pub use node::TestNode;

mod setup;
pub use setup::{TestConfig, TestSetup};

#[cfg(test)]
mod tests;
