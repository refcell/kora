//! Error types for QMDB operations.

use kora_primitives::B256;
use thiserror::Error;

/// Error type for QMDB operations, implementing REVM's DBErrorMarker.
#[derive(Debug, Error)]
pub enum QmdbError {
    /// Storage operation failed.
    #[error("storage error: {0}")]
    Storage(String),

    /// Code not found for the given hash.
    #[error("code not found: {0}")]
    CodeNotFound(B256),

    /// Block hash not found for the given number.
    #[error("block hash not found: {0}")]
    BlockHashNotFound(u64),
}

impl revm::database_interface::DBErrorMarker for QmdbError {}
