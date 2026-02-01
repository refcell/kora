//! Indexed state provider for production RPC use.
//!
//! Integrates the block indexer with ledger state to provide a complete
//! [`StateProvider`] implementation for RPC queries.

use std::sync::Arc;

use alloy_primitives::{Address, B256, Bytes, U64, U256};
use async_trait::async_trait;
use kora_indexer::{BlockIndex, IndexedBlock, IndexedReceipt, IndexedTransaction, LogFilter};
use kora_traits::{StateDbError, StateDbRead};

use crate::{
    error::RpcError,
    state_provider::StateProvider,
    types::{
        BlockNumberOrTag, BlockTag, BlockTransactions, RpcBlock, RpcLog, RpcLogFilter,
        RpcTransaction, RpcTransactionReceipt,
    },
};

/// State provider that combines indexed block data with live state queries.
///
/// Uses [`BlockIndex`] for block, transaction, and receipt lookups, and
/// delegates account state queries (balance, nonce, code, storage) to
/// a generic state database implementation.
#[derive(Debug)]
pub struct IndexedStateProvider<S> {
    index: Arc<BlockIndex>,
    state: S,
}

impl<S> IndexedStateProvider<S> {
    /// Creates a new indexed state provider.
    pub const fn new(index: Arc<BlockIndex>, state: S) -> Self {
        Self { index, state }
    }
}

impl<S: Clone> Clone for IndexedStateProvider<S> {
    fn clone(&self) -> Self {
        Self { index: Arc::clone(&self.index), state: self.state.clone() }
    }
}

#[async_trait]
impl<S: StateDbRead + Send + Sync + 'static> StateProvider for IndexedStateProvider<S> {
    async fn balance(
        &self,
        address: Address,
        _block: Option<BlockNumberOrTag>,
    ) -> Result<U256, RpcError> {
        self.state.balance(&address).await.map_err(state_error_to_rpc)
    }

    async fn nonce(
        &self,
        address: Address,
        _block: Option<BlockNumberOrTag>,
    ) -> Result<u64, RpcError> {
        self.state.nonce(&address).await.map_err(state_error_to_rpc)
    }

    async fn code(
        &self,
        address: Address,
        _block: Option<BlockNumberOrTag>,
    ) -> Result<Bytes, RpcError> {
        let code_hash = self.state.code_hash(&address).await.map_err(state_error_to_rpc)?;
        self.state.code(&code_hash).await.map_err(state_error_to_rpc)
    }

    async fn storage(
        &self,
        address: Address,
        slot: U256,
        _block: Option<BlockNumberOrTag>,
    ) -> Result<U256, RpcError> {
        self.state.storage(&address, &slot).await.map_err(state_error_to_rpc)
    }

    async fn block_by_number(&self, block: BlockNumberOrTag) -> Result<Option<RpcBlock>, RpcError> {
        let block_num = self.resolve_block_number(&block)?;
        let indexed = self.index.get_block_by_number(block_num);
        Ok(indexed.map(indexed_block_to_rpc))
    }

    async fn block_by_hash(&self, hash: B256) -> Result<Option<RpcBlock>, RpcError> {
        let indexed = self.index.get_block_by_hash(&hash);
        Ok(indexed.map(indexed_block_to_rpc))
    }

    async fn transaction_by_hash(&self, hash: B256) -> Result<Option<RpcTransaction>, RpcError> {
        let indexed = self.index.get_transaction(&hash);
        Ok(indexed.map(indexed_tx_to_rpc))
    }

    async fn receipt_by_hash(&self, hash: B256) -> Result<Option<RpcTransactionReceipt>, RpcError> {
        let indexed = self.index.get_receipt(&hash);
        Ok(indexed.map(indexed_receipt_to_rpc))
    }

    async fn block_number(&self) -> Result<u64, RpcError> {
        Ok(self.index.head_block_number())
    }

    async fn get_logs(&self, filter: RpcLogFilter) -> Result<Vec<RpcLog>, RpcError> {
        let from_block =
            filter.from_block.as_ref().map(|b| self.resolve_block_number(b)).transpose()?;
        let to_block =
            filter.to_block.as_ref().map(|b| self.resolve_block_number(b)).transpose()?;

        let mut log_filter = LogFilter::new();
        if let Some(from) = from_block {
            log_filter = log_filter.from_block(from);
        }
        if let Some(to) = to_block {
            log_filter = log_filter.to_block(to);
        }
        if let Some(addr_filter) = filter.address {
            log_filter = log_filter.address(addr_filter.into_vec());
        }
        if let Some(topics) = filter.topics {
            for (i, topic_filter) in topics.into_iter().enumerate() {
                if let Some(tf) = topic_filter {
                    log_filter = log_filter.topic(i, tf.into_vec());
                }
            }
        }

        let indexed_logs = self.index.get_logs(&log_filter);
        let block_number = self.index.head_block_number();
        let logs = indexed_logs
            .into_iter()
            .map(|log| RpcLog {
                address: log.address,
                topics: log.topics,
                data: log.data,
                block_number: U64::from(block_number),
                transaction_hash: B256::ZERO,
                transaction_index: U64::ZERO,
                block_hash: B256::ZERO,
                log_index: U64::from(log.log_index),
                removed: false,
            })
            .collect();
        Ok(logs)
    }
}

impl<S> IndexedStateProvider<S> {
    fn resolve_block_number(&self, block: &BlockNumberOrTag) -> Result<u64, RpcError> {
        match block {
            BlockNumberOrTag::Number(n) => Ok(n.to::<u64>()),
            BlockNumberOrTag::Tag(tag) => self.resolve_tag(*tag),
            BlockNumberOrTag::Latest => Ok(self.index.head_block_number()),
        }
    }

    fn resolve_tag(&self, tag: BlockTag) -> Result<u64, RpcError> {
        match tag {
            BlockTag::Latest | BlockTag::Safe | BlockTag::Finalized | BlockTag::Pending => {
                Ok(self.index.head_block_number())
            }
            BlockTag::Earliest => Ok(0),
        }
    }
}

fn state_error_to_rpc(err: StateDbError) -> RpcError {
    match err {
        StateDbError::AccountNotFound(addr) => RpcError::AccountNotFound(addr.to_string()),
        StateDbError::CodeNotFound(hash) => RpcError::StateError(format!("code not found: {hash}")),
        StateDbError::Storage(msg) => RpcError::StateError(msg),
        StateDbError::LockPoisoned => RpcError::Internal("lock poisoned".to_string()),
        StateDbError::RootComputation(msg) => RpcError::StateError(msg),
    }
}

fn indexed_block_to_rpc(block: IndexedBlock) -> RpcBlock {
    RpcBlock {
        hash: block.hash,
        parent_hash: block.parent_hash,
        number: U64::from(block.number),
        state_root: block.state_root,
        transactions_root: B256::ZERO,
        receipts_root: B256::ZERO,
        logs_bloom: Bytes::new(),
        timestamp: U64::from(block.timestamp),
        gas_limit: U64::from(block.gas_limit),
        gas_used: U64::from(block.gas_used),
        extra_data: Bytes::new(),
        mix_hash: B256::ZERO,
        nonce: Default::default(),
        base_fee_per_gas: block.base_fee_per_gas.map(U256::from),
        miner: Address::ZERO,
        difficulty: U256::ZERO,
        total_difficulty: U256::ZERO,
        uncles: vec![],
        size: U64::ZERO,
        transactions: BlockTransactions::Hashes(block.transaction_hashes),
    }
}

fn indexed_tx_to_rpc(tx: IndexedTransaction) -> RpcTransaction {
    RpcTransaction {
        hash: tx.hash,
        nonce: U64::from(tx.nonce),
        block_hash: Some(tx.block_hash),
        block_number: Some(U64::from(tx.block_number)),
        transaction_index: Some(U64::from(tx.index)),
        from: tx.from,
        to: tx.to,
        value: tx.value,
        gas: U64::from(tx.gas_limit),
        gas_price: U256::from(tx.gas_price),
        input: tx.input,
        tx_type: U64::ZERO,
        chain_id: None,
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
        v: U64::ZERO,
        r: U256::ZERO,
        s: U256::ZERO,
    }
}

fn indexed_receipt_to_rpc(receipt: IndexedReceipt) -> RpcTransactionReceipt {
    let logs = receipt
        .logs
        .into_iter()
        .map(|log| RpcLog {
            address: log.address,
            topics: log.topics,
            data: log.data,
            block_number: U64::from(receipt.block_number),
            transaction_hash: receipt.transaction_hash,
            transaction_index: U64::from(receipt.transaction_index),
            block_hash: receipt.block_hash,
            log_index: U64::from(log.log_index),
            removed: false,
        })
        .collect();

    RpcTransactionReceipt {
        transaction_hash: receipt.transaction_hash,
        transaction_index: U64::from(receipt.transaction_index),
        block_hash: receipt.block_hash,
        block_number: U64::from(receipt.block_number),
        from: receipt.from,
        to: receipt.to,
        cumulative_gas_used: U64::from(receipt.cumulative_gas_used),
        gas_used: U64::from(receipt.gas_used),
        contract_address: receipt.contract_address,
        logs,
        logs_bloom: Bytes::new(),
        tx_type: U64::ZERO,
        status: if receipt.status { U64::from(1) } else { U64::ZERO },
        effective_gas_price: U256::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use kora_indexer::IndexedLog;

    use super::*;

    #[derive(Clone)]
    struct MockState;

    impl StateDbRead for MockState {
        async fn nonce(&self, _address: &Address) -> Result<u64, StateDbError> {
            Ok(42)
        }

        async fn balance(&self, _address: &Address) -> Result<U256, StateDbError> {
            Ok(U256::from(1000))
        }

        async fn code_hash(&self, _address: &Address) -> Result<B256, StateDbError> {
            Ok(B256::ZERO)
        }

        async fn code(&self, _code_hash: &B256) -> Result<Bytes, StateDbError> {
            Ok(Bytes::from_static(&[0x60, 0x00]))
        }

        async fn storage(&self, _address: &Address, _slot: &U256) -> Result<U256, StateDbError> {
            Ok(U256::from(123))
        }
    }

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
            logs: vec![IndexedLog {
                address: Address::ZERO,
                topics: vec![],
                data: Bytes::new(),
                log_index: 0,
            }],
            status: true,
        }
    }

    #[tokio::test]
    async fn test_balance() {
        let index = Arc::new(BlockIndex::new());
        let provider = IndexedStateProvider::new(index, MockState);

        let balance = provider.balance(Address::ZERO, None).await.unwrap();
        assert_eq!(balance, U256::from(1000));
    }

    #[tokio::test]
    async fn test_nonce() {
        let index = Arc::new(BlockIndex::new());
        let provider = IndexedStateProvider::new(index, MockState);

        let nonce = provider.nonce(Address::ZERO, None).await.unwrap();
        assert_eq!(nonce, 42);
    }

    #[tokio::test]
    async fn test_block_by_number() {
        let index = Arc::new(BlockIndex::new());
        let block_hash = B256::repeat_byte(1);
        index.insert_block(create_test_block(1, block_hash), vec![], vec![]);

        let provider = IndexedStateProvider::new(index, MockState);

        let block = provider.block_by_number(BlockNumberOrTag::Number(U64::from(1))).await.unwrap();
        assert!(block.is_some());
        assert_eq!(block.unwrap().hash, block_hash);
    }

    #[tokio::test]
    async fn test_block_by_hash() {
        let index = Arc::new(BlockIndex::new());
        let block_hash = B256::repeat_byte(1);
        index.insert_block(create_test_block(1, block_hash), vec![], vec![]);

        let provider = IndexedStateProvider::new(index, MockState);

        let block = provider.block_by_hash(block_hash).await.unwrap();
        assert!(block.is_some());
        assert_eq!(block.unwrap().number, U64::from(1));
    }

    #[tokio::test]
    async fn test_transaction_by_hash() {
        let index = Arc::new(BlockIndex::new());
        let block_hash = B256::repeat_byte(1);
        let tx_hash = B256::repeat_byte(2);
        index.insert_block(
            create_test_block(1, block_hash),
            vec![create_test_tx(tx_hash, block_hash, 1)],
            vec![],
        );

        let provider = IndexedStateProvider::new(index, MockState);

        let tx = provider.transaction_by_hash(tx_hash).await.unwrap();
        assert!(tx.is_some());
        assert_eq!(tx.unwrap().hash, tx_hash);
    }

    #[tokio::test]
    async fn test_receipt_by_hash() {
        let index = Arc::new(BlockIndex::new());
        let block_hash = B256::repeat_byte(1);
        let tx_hash = B256::repeat_byte(2);
        index.insert_block(
            create_test_block(1, block_hash),
            vec![create_test_tx(tx_hash, block_hash, 1)],
            vec![create_test_receipt(tx_hash, block_hash, 1)],
        );

        let provider = IndexedStateProvider::new(index, MockState);

        let receipt = provider.receipt_by_hash(tx_hash).await.unwrap();
        assert!(receipt.is_some());
        let receipt = receipt.unwrap();
        assert_eq!(receipt.transaction_hash, tx_hash);
        assert_eq!(receipt.logs.len(), 1);
    }

    #[tokio::test]
    async fn test_block_number() {
        let index = Arc::new(BlockIndex::new());
        index.insert_block(create_test_block(5, B256::repeat_byte(5)), vec![], vec![]);

        let provider = IndexedStateProvider::new(index, MockState);

        let num = provider.block_number().await.unwrap();
        assert_eq!(num, 5);
    }

    #[tokio::test]
    async fn test_resolve_block_tags() {
        let index = Arc::new(BlockIndex::new());
        index.insert_block(create_test_block(10, B256::repeat_byte(10)), vec![], vec![]);

        let provider = IndexedStateProvider::new(index, MockState);

        let block =
            provider.block_by_number(BlockNumberOrTag::Tag(BlockTag::Latest)).await.unwrap();
        assert!(block.is_some());
        assert_eq!(block.unwrap().number, U64::from(10));

        let block =
            provider.block_by_number(BlockNumberOrTag::Tag(BlockTag::Earliest)).await.unwrap();
        assert!(block.is_none());
    }
}
