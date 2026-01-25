#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/anthropics/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub use alloy_primitives::{
    Address, B256, Bytes, KECCAK256_EMPTY, U256,
    map::{DefaultHashBuilder, HashMap},
};
