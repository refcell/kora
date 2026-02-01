#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
// Trait signatures require impl Future return type
#![allow(clippy::manual_async_fn)]

mod app;
pub use app::RevmApplication;

mod error;
pub use error::RunnerError;

mod runner;
pub use runner::ProductionRunner;

mod scheme;
pub use scheme::{ThresholdScheme, load_threshold_scheme};
