//! Block dissemination handler for Kora using commonware Marshaled adapter.

#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod actor;
pub use actor::ActorInitializer;

mod archive;
pub use archive::ArchiveInitializer;

mod broadcast;
pub use broadcast::BroadcastInitializer;

mod peers;
pub use peers::PeerInitializer;
