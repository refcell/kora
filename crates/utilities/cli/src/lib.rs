//! Minimal CLI utilities for the Kora binary.

#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod backtrace;
pub use backtrace::Backtracing;

#[cfg(unix)]
mod sigsegv;
#[cfg(unix)]
pub use sigsegv::SigsegvHandler;
