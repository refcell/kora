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
    pub const fn from_block(mut self, block: u64) -> Self {
        self.from_block = Some(block);
        self
    }

    /// Sets the end block.
    pub const fn to_block(mut self, block: u64) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_filter_new() {
        let filter = LogFilter::new();
        assert!(filter.from_block.is_none());
        assert!(filter.to_block.is_none());
        assert!(filter.address.is_none());
        assert!(filter.topics.iter().all(|t| t.is_none()));
    }

    #[test]
    fn log_filter_default() {
        let filter = LogFilter::default();
        assert!(filter.from_block.is_none());
        assert!(filter.to_block.is_none());
        assert!(filter.address.is_none());
        assert!(filter.topics.iter().all(|t| t.is_none()));
    }

    #[test]
    fn log_filter_from_block() {
        let filter = LogFilter::new().from_block(100);
        assert_eq!(filter.from_block, Some(100));
        assert!(filter.to_block.is_none());
    }

    #[test]
    fn log_filter_to_block() {
        let filter = LogFilter::new().to_block(200);
        assert!(filter.from_block.is_none());
        assert_eq!(filter.to_block, Some(200));
    }

    #[test]
    fn log_filter_block_range() {
        let filter = LogFilter::new().from_block(10).to_block(20);
        assert_eq!(filter.from_block, Some(10));
        assert_eq!(filter.to_block, Some(20));
    }

    #[test]
    fn log_filter_address() {
        let addr1 = Address::repeat_byte(0x01);
        let addr2 = Address::repeat_byte(0x02);
        let filter = LogFilter::new().address(vec![addr1, addr2]);

        let addrs = filter.address.unwrap();
        assert_eq!(addrs.len(), 2);
        assert_eq!(addrs[0], addr1);
        assert_eq!(addrs[1], addr2);
    }

    #[test]
    fn log_filter_topic_at_index() {
        let topic = B256::repeat_byte(0xab);
        let filter = LogFilter::new().topic(0, vec![topic]);

        assert!(filter.topics[0].is_some());
        assert!(filter.topics[1].is_none());
        assert!(filter.topics[2].is_none());
        assert!(filter.topics[3].is_none());
        assert_eq!(filter.topics[0].as_ref().unwrap()[0], topic);
    }

    #[test]
    fn log_filter_multiple_topics() {
        let topic0 = B256::repeat_byte(0x01);
        let topic1 = B256::repeat_byte(0x02);
        let topic2 = B256::repeat_byte(0x03);

        let filter = LogFilter::new()
            .topic(0, vec![topic0])
            .topic(1, vec![topic1])
            .topic(2, vec![topic2]);

        assert!(filter.topics[0].is_some());
        assert!(filter.topics[1].is_some());
        assert!(filter.topics[2].is_some());
        assert!(filter.topics[3].is_none());
    }

    #[test]
    fn log_filter_topic_out_of_bounds_ignored() {
        let topic = B256::repeat_byte(0xff);
        let filter = LogFilter::new().topic(5, vec![topic]);

        assert!(filter.topics.iter().all(|t| t.is_none()));
    }

    #[test]
    fn log_filter_chained_builder() {
        let addr = Address::repeat_byte(0x42);
        let topic = B256::repeat_byte(0xcd);

        let filter = LogFilter::new()
            .from_block(100)
            .to_block(200)
            .address(vec![addr])
            .topic(0, vec![topic]);

        assert_eq!(filter.from_block, Some(100));
        assert_eq!(filter.to_block, Some(200));
        assert!(filter.address.is_some());
        assert!(filter.topics[0].is_some());
    }
}
