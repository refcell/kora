#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
// DKG implementation is work-in-progress
#![allow(dead_code, missing_docs, missing_debug_implementations, unreachable_pub)]

#[cfg(test)]
mod tests;

mod ceremony;
pub use ceremony::DkgCeremony;

mod config;
pub use config::DkgConfig;

mod error;
pub use error::DkgError;

mod network;
pub use network::DkgNetwork;

mod output;
pub use output::DkgOutput;

mod protocol;
pub use protocol::{CeremonySession, DkgParticipant, ProtocolMessage, ProtocolMessageKind};

mod state;
pub use state::{DkgPhase, PersistedDkgState};

mod transport;
pub use transport::{
    CHANNEL_DKG, DEFAULT_BACKLOG, DEFAULT_MAX_MESSAGE_SIZE, DKG_NAMESPACE, DkgReceiver, DkgSender,
    DkgTransport, DkgTransportConfig,
};
