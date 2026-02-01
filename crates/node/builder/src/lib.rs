//! Node builder for constructing Kora nodes with consensus components.
#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![feature(associated_type_defaults)]

mod builder;
pub use builder::NodeBuilder;

mod traits;
pub use traits::{ConsensusProvider, NodeComponents, Random};
