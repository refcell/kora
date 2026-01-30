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
    /// Parent block hash.
    pub parent_hash: B256,
    /// Previous block's randomness (prevrandao).
    pub prevrandao: B256,
    /// Blob base fee for Cancun+ (EIP-4844).
    pub blob_base_fee: Option<u128>,
}

impl BlockContext {
    /// Create a new block context.
    pub const fn new(header: Header, parent_hash: B256, prevrandao: B256) -> Self {
        Self { header, parent_hash, prevrandao, blob_base_fee: None }
    }

    /// Set the blob base fee.
    pub const fn with_blob_base_fee(mut self, blob_base_fee: u128) -> Self {
        self.blob_base_fee = Some(blob_base_fee);
        self
    }

    /// Get the base fee from the header.
    pub fn base_fee(&self) -> u64 {
        self.header.base_fee_per_gas.unwrap_or_default()
    }
}

/// Parent block info for header validation.
#[derive(Clone, Debug)]
pub struct ParentBlock {
    /// Parent block hash.
    pub hash: B256,
    /// Parent block number.
    pub number: u64,
    /// Parent block timestamp.
    pub timestamp: u64,
    /// Parent gas limit.
    pub gas_limit: u64,
    /// Parent gas used.
    pub gas_used: u64,
    /// Parent base fee per gas (EIP-1559).
    pub base_fee_per_gas: Option<u64>,
}

impl ParentBlock {
    /// Create parent block info from a header.
    pub fn from_header(header: &Header, hash: B256) -> Self {
        Self {
            hash,
            number: header.number,
            timestamp: header.timestamp,
            gas_limit: header.gas_limit,
            gas_used: header.gas_used,
            base_fee_per_gas: header.base_fee_per_gas,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_context_new() {
        let header = Header::default();
        let parent_hash = B256::repeat_byte(1);
        let prevrandao = B256::ZERO;
        let context = BlockContext::new(header, parent_hash, prevrandao);
        assert_eq!(context.prevrandao, B256::ZERO);
        assert_eq!(context.parent_hash, parent_hash);
        assert!(context.blob_base_fee.is_none());
    }

    #[test]
    fn block_context_with_blob_base_fee() {
        let header = Header::default();
        let context = BlockContext::new(header, B256::ZERO, B256::ZERO)
            .with_blob_base_fee(1000);
        assert_eq!(context.blob_base_fee, Some(1000));
    }

    #[test]
    fn parent_block_from_header() {
        let mut header = Header::default();
        header.number = 100;
        header.timestamp = 1234567890;
        header.gas_limit = 30_000_000;
        header.gas_used = 15_000_000;
        header.base_fee_per_gas = Some(1000);

        let hash = B256::repeat_byte(0xab);
        let parent = ParentBlock::from_header(&header, hash);

        assert_eq!(parent.hash, hash);
        assert_eq!(parent.number, 100);
        assert_eq!(parent.timestamp, 1234567890);
        assert_eq!(parent.gas_limit, 30_000_000);
        assert_eq!(parent.gas_used, 15_000_000);
        assert_eq!(parent.base_fee_per_gas, Some(1000));
    }
}
