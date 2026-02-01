//! In-memory block index storage.

use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};

use alloy_primitives::B256;
use parking_lot::RwLock;
use tracing::debug;

use crate::{
    filter::LogFilter,
    types::{IndexStats, IndexedBlock, IndexedLog, IndexedReceipt, IndexedTransaction},
};

/// In-memory storage for indexed blocks, transactions, receipts, and logs.
#[derive(Debug)]
pub struct BlockIndex {
    blocks_by_hash: RwLock<HashMap<B256, IndexedBlock>>,
    blocks_by_number: RwLock<HashMap<u64, B256>>,
    transactions: RwLock<HashMap<B256, IndexedTransaction>>,
    receipts: RwLock<HashMap<B256, IndexedReceipt>>,
    logs_by_block: RwLock<HashMap<B256, Vec<IndexedLog>>>,
    head_block: AtomicU64,
}

impl Default for BlockIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockIndex {
    /// Creates a new empty block index.
    pub fn new() -> Self {
        Self {
            blocks_by_hash: RwLock::new(HashMap::new()),
            blocks_by_number: RwLock::new(HashMap::new()),
            transactions: RwLock::new(HashMap::new()),
            receipts: RwLock::new(HashMap::new()),
            logs_by_block: RwLock::new(HashMap::new()),
            head_block: AtomicU64::new(0),
        }
    }

    /// Inserts a block with its transactions and receipts into the index.
    pub fn insert_block(
        &self,
        block: IndexedBlock,
        txs: Vec<IndexedTransaction>,
        receipts: Vec<IndexedReceipt>,
    ) {
        let block_hash = block.hash;
        let block_number = block.number;

        debug!(number = block_number, hash = %block_hash, txs = txs.len(), "indexing block");

        let mut all_logs = Vec::new();
        for receipt in &receipts {
            all_logs.extend(receipt.logs.clone());
        }

        {
            let mut blocks_by_hash = self.blocks_by_hash.write();
            blocks_by_hash.insert(block_hash, block);
        }

        {
            let mut blocks_by_number = self.blocks_by_number.write();
            blocks_by_number.insert(block_number, block_hash);
        }

        {
            let mut transactions = self.transactions.write();
            for tx in txs {
                transactions.insert(tx.hash, tx);
            }
        }

        {
            let mut receipts_map = self.receipts.write();
            for receipt in receipts {
                receipts_map.insert(receipt.transaction_hash, receipt);
            }
        }

        {
            let mut logs_by_block = self.logs_by_block.write();
            logs_by_block.insert(block_hash, all_logs);
        }

        let mut current = self.head_block.load(Ordering::Acquire);
        while block_number > current {
            match self.head_block.compare_exchange_weak(
                current,
                block_number,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(c) => current = c,
            }
        }
    }

    /// Gets a block by its hash.
    pub fn get_block_by_hash(&self, hash: &B256) -> Option<IndexedBlock> {
        self.blocks_by_hash.read().get(hash).cloned()
    }

    /// Gets a block by its number.
    pub fn get_block_by_number(&self, number: u64) -> Option<IndexedBlock> {
        let blocks_by_number = self.blocks_by_number.read();
        let hash = blocks_by_number.get(&number)?;
        self.blocks_by_hash.read().get(hash).cloned()
    }

    /// Gets a transaction by its hash.
    pub fn get_transaction(&self, hash: &B256) -> Option<IndexedTransaction> {
        self.transactions.read().get(hash).cloned()
    }

    /// Gets a receipt by its transaction hash.
    pub fn get_receipt(&self, hash: &B256) -> Option<IndexedReceipt> {
        self.receipts.read().get(hash).cloned()
    }

    /// Returns the current head block number.
    #[must_use]
    pub fn head_block_number(&self) -> u64 {
        self.head_block.load(Ordering::Acquire)
    }

    /// Gets logs matching the given filter.
    pub fn get_logs(&self, filter: &LogFilter) -> Vec<IndexedLog> {
        let from_block = filter.from_block.unwrap_or(0);
        let to_block = filter.to_block.unwrap_or_else(|| self.head_block_number());

        let mut result = Vec::new();

        let blocks_by_number = self.blocks_by_number.read();
        let logs_by_block = self.logs_by_block.read();

        for block_num in from_block..=to_block {
            let Some(block_hash) = blocks_by_number.get(&block_num) else {
                continue;
            };

            let Some(logs) = logs_by_block.get(block_hash) else {
                continue;
            };

            for log in logs {
                if !Self::matches_filter(log, filter) {
                    continue;
                }
                result.push(log.clone());
            }
        }

        result
    }

    /// Returns the total number of indexed blocks.
    #[must_use]
    pub fn block_count(&self) -> usize {
        self.blocks_by_hash.read().len()
    }

    /// Returns the total number of indexed transactions.
    #[must_use]
    pub fn transaction_count(&self) -> usize {
        self.transactions.read().len()
    }

    /// Returns the total number of indexed receipts.
    #[must_use]
    pub fn receipt_count(&self) -> usize {
        self.receipts.read().len()
    }

    /// Returns true if the index is empty (no blocks indexed).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.blocks_by_hash.read().is_empty()
    }

    /// Returns statistics about the index.
    #[must_use]
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            block_count: self.block_count(),
            transaction_count: self.transaction_count(),
            receipt_count: self.receipt_count(),
            head_block_number: self.head_block_number(),
        }
    }

    fn matches_filter(log: &IndexedLog, filter: &LogFilter) -> bool {
        if let Some(addresses) = &filter.address
            && !addresses.contains(&log.address)
        {
            return false;
        }

        for (i, topic_filter) in filter.topics.iter().enumerate() {
            if let Some(allowed_topics) = topic_filter {
                match log.topics.get(i) {
                    Some(log_topic) if allowed_topics.contains(log_topic) => {}
                    _ => return false,
                }
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, B256, Bytes, U256};

    use super::*;

    fn create_test_block(number: u64, hash: B256) -> IndexedBlock {
        IndexedBlock {
            hash,
            number,
            parent_hash: B256::ZERO,
            state_root: B256::ZERO,
            timestamp: 1000 + number,
            gas_limit: 30_000_000,
            gas_used: 21_000,
            base_fee_per_gas: Some(1_000_000_000),
            transaction_hashes: vec![],
        }
    }

    fn create_test_tx(hash: B256, block_hash: B256, block_number: u64) -> IndexedTransaction {
        IndexedTransaction {
            hash,
            block_hash,
            block_number,
            index: 0,
            from: Address::ZERO,
            to: Some(Address::ZERO),
            value: U256::ZERO,
            gas_limit: 21_000,
            gas_price: 1_000_000_000,
            input: Bytes::new(),
            nonce: 0,
        }
    }

    fn create_test_receipt(tx_hash: B256, block_hash: B256, block_number: u64) -> IndexedReceipt {
        IndexedReceipt {
            transaction_hash: tx_hash,
            block_hash,
            block_number,
            transaction_index: 0,
            from: Address::ZERO,
            to: Some(Address::ZERO),
            cumulative_gas_used: 21_000,
            gas_used: 21_000,
            contract_address: None,
            logs: vec![],
            status: true,
        }
    }

    #[test]
    fn test_insert_and_get_block() {
        let index = BlockIndex::new();
        let block_hash = B256::repeat_byte(1);
        let block = create_test_block(1, block_hash);

        index.insert_block(block, vec![], vec![]);

        let retrieved = index.get_block_by_hash(&block_hash).unwrap();
        assert_eq!(retrieved.number, 1);
        assert_eq!(retrieved.hash, block_hash);

        let by_number = index.get_block_by_number(1).unwrap();
        assert_eq!(by_number.hash, block_hash);
    }

    #[test]
    fn test_insert_and_get_transaction() {
        let index = BlockIndex::new();
        let block_hash = B256::repeat_byte(1);
        let tx_hash = B256::repeat_byte(2);
        let block = create_test_block(1, block_hash);
        let tx = create_test_tx(tx_hash, block_hash, 1);
        let receipt = create_test_receipt(tx_hash, block_hash, 1);

        index.insert_block(block, vec![tx], vec![receipt]);

        let retrieved_tx = index.get_transaction(&tx_hash).unwrap();
        assert_eq!(retrieved_tx.hash, tx_hash);

        let retrieved_receipt = index.get_receipt(&tx_hash).unwrap();
        assert_eq!(retrieved_receipt.transaction_hash, tx_hash);
    }

    #[test]
    fn test_head_block_number() {
        let index = BlockIndex::new();
        assert_eq!(index.head_block_number(), 0);

        index.insert_block(create_test_block(5, B256::repeat_byte(5)), vec![], vec![]);
        assert_eq!(index.head_block_number(), 5);

        index.insert_block(create_test_block(3, B256::repeat_byte(3)), vec![], vec![]);
        assert_eq!(index.head_block_number(), 5);

        index.insert_block(create_test_block(10, B256::repeat_byte(10)), vec![], vec![]);
        assert_eq!(index.head_block_number(), 10);
    }

    #[test]
    fn test_get_logs_with_filter() {
        let index = BlockIndex::new();
        let block_hash = B256::repeat_byte(1);
        let contract_addr = Address::repeat_byte(0xAB);
        let topic = B256::repeat_byte(0xCD);

        let log = IndexedLog {
            address: contract_addr,
            topics: vec![topic],
            data: Bytes::new(),
            log_index: 0,
        };

        let receipt = IndexedReceipt {
            transaction_hash: B256::repeat_byte(2),
            block_hash,
            block_number: 1,
            transaction_index: 0,
            from: Address::ZERO,
            to: None,
            cumulative_gas_used: 21_000,
            gas_used: 21_000,
            contract_address: None,
            logs: vec![log],
            status: true,
        };

        index.insert_block(create_test_block(1, block_hash), vec![], vec![receipt]);

        let filter = LogFilter::new().address(vec![contract_addr]);
        let logs = index.get_logs(&filter);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].address, contract_addr);

        let filter = LogFilter::new().topic(0, vec![topic]);
        let logs = index.get_logs(&filter);
        assert_eq!(logs.len(), 1);

        let filter = LogFilter::new().address(vec![Address::repeat_byte(0xFF)]);
        let logs = index.get_logs(&filter);
        assert!(logs.is_empty());
    }

    #[test]
    fn test_is_empty() {
        let index = BlockIndex::new();
        assert!(index.is_empty());

        index.insert_block(create_test_block(1, B256::repeat_byte(1)), vec![], vec![]);
        assert!(!index.is_empty());
    }

    #[test]
    fn test_block_count() {
        let index = BlockIndex::new();
        assert_eq!(index.block_count(), 0);

        index.insert_block(create_test_block(1, B256::repeat_byte(1)), vec![], vec![]);
        assert_eq!(index.block_count(), 1);

        index.insert_block(create_test_block(2, B256::repeat_byte(2)), vec![], vec![]);
        assert_eq!(index.block_count(), 2);
    }

    #[test]
    fn test_transaction_count() {
        let index = BlockIndex::new();
        assert_eq!(index.transaction_count(), 0);

        let block_hash = B256::repeat_byte(1);
        let tx1 = create_test_tx(B256::repeat_byte(2), block_hash, 1);
        let tx2 = create_test_tx(B256::repeat_byte(3), block_hash, 1);

        index.insert_block(create_test_block(1, block_hash), vec![tx1, tx2], vec![]);
        assert_eq!(index.transaction_count(), 2);
    }

    #[test]
    fn test_receipt_count() {
        let index = BlockIndex::new();
        assert_eq!(index.receipt_count(), 0);

        let block_hash = B256::repeat_byte(1);
        let tx_hash = B256::repeat_byte(2);
        let receipt = create_test_receipt(tx_hash, block_hash, 1);

        index.insert_block(create_test_block(1, block_hash), vec![], vec![receipt]);
        assert_eq!(index.receipt_count(), 1);
    }

    #[test]
    fn test_stats() {
        let index = BlockIndex::new();

        let stats = index.stats();
        assert_eq!(stats.block_count, 0);
        assert_eq!(stats.transaction_count, 0);
        assert_eq!(stats.receipt_count, 0);
        assert_eq!(stats.head_block_number, 0);

        let block_hash = B256::repeat_byte(1);
        let tx_hash = B256::repeat_byte(2);
        let tx = create_test_tx(tx_hash, block_hash, 5);
        let receipt = create_test_receipt(tx_hash, block_hash, 5);

        index.insert_block(create_test_block(5, block_hash), vec![tx], vec![receipt]);

        let stats = index.stats();
        assert_eq!(stats.block_count, 1);
        assert_eq!(stats.transaction_count, 1);
        assert_eq!(stats.receipt_count, 1);
        assert_eq!(stats.head_block_number, 5);
    }
}
