//! Simulated P2P transport for Kora node testing and development.

#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod channels;
pub use channels::{Receiver, Sender, SimMarshalChannels, SimSimplexChannels};

mod context;
pub use context::SimContext;

mod error;
pub use error::SimTransportError;

mod provider;
pub use provider::{
    SimChannels, SimControl, SimLinkConfig, SimTransportProvider, create_sim_network,
    register_node_channels,
};
