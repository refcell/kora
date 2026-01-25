//! Block execution context.

use alloy_consensus::Header;
use alloy_primitives::B256;

/// Context for block execution.
///
/// Contains the block header and additional execution parameters.
#[derive(Clone, Debug)]
pub struct BlockContext {
    /// Block header.
    pub header: Header,
    /// Previous block's randomness (prevrandao).
    pub prevrandao: B256,
}

impl BlockContext {
    /// Create a new block context.
    pub const fn new(header: Header, prevrandao: B256) -> Self {
        Self { header, prevrandao }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_context_new() {
        let header = Header::default();
        let prevrandao = B256::ZERO;
        let context = BlockContext::new(header, prevrandao);
        assert_eq!(context.prevrandao, B256::ZERO);
    }
}
