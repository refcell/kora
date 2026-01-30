//! Log filtering utilities.

use alloy_primitives::{Address, B256};

/// A filter for querying logs.
#[derive(Debug, Clone, Default)]
pub struct LogFilter {
    /// Start block (inclusive).
    pub from_block: Option<u64>,
    /// End block (inclusive).
    pub to_block: Option<u64>,
    /// Filter by contract addresses (OR logic).
    pub address: Option<Vec<Address>>,
    /// Filter by topics. Each position uses OR logic within, AND logic across positions.
    pub topics: [Option<Vec<B256>>; 4],
}

impl LogFilter {
    /// Creates a new empty log filter.
    pub const fn new() -> Self {
        Self { from_block: None, to_block: None, address: None, topics: [None, None, None, None] }
    }

    /// Sets the start block.
    pub fn from_block(mut self, block: u64) -> Self {
        self.from_block = Some(block);
        self
    }

    /// Sets the end block.
    pub fn to_block(mut self, block: u64) -> Self {
        self.to_block = Some(block);
        self
    }

    /// Sets the address filter.
    pub fn address(mut self, addresses: Vec<Address>) -> Self {
        self.address = Some(addresses);
        self
    }

    /// Sets a topic filter at the given index.
    pub fn topic(mut self, index: usize, topics: Vec<B256>) -> Self {
        if index < 4 {
            self.topics[index] = Some(topics);
        }
        self
    }
}
