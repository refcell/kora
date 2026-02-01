//! P2P transport layer for Kora nodes.

#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod builder;

mod bundle;
pub use bundle::TransportBundle;

mod channels;
pub use channels::{
    CHANNEL_BACKFILL, CHANNEL_BLOCKS, CHANNEL_CERTS, CHANNEL_RESOLVER, CHANNEL_VOTES,
    MarshalChannels, Receiver, Sender, SimplexChannels,
};

mod config;
pub use config::{
    DEFAULT_BACKLOG, DEFAULT_MAX_MESSAGE_SIZE, DEFAULT_NAMESPACE, TransportConfig, TransportParsing,
};

mod error;
pub use error::TransportError;

mod ext;
pub use ext::NetworkConfigExt;

mod provider;
pub use provider::TransportProvider;

mod network_provider;
pub use network_provider::{NetworkControl, NetworkTransportProvider};

mod transport;
pub use transport::NetworkTransport;
