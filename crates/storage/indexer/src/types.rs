//! Indexed types for blocks, transactions, receipts, and logs.

use alloy_primitives::{Address, Bytes, B256, U256};

/// An indexed block containing header information and transaction hashes.
#[derive(Debug, Clone)]
pub struct IndexedBlock {
    /// Block hash.
    pub hash: B256,
    /// Block number.
    pub number: u64,
    /// Parent block hash.
    pub parent_hash: B256,
    /// State root after executing this block.
    pub state_root: B256,
    /// Block timestamp.
    pub timestamp: u64,
    /// Gas limit for this block.
    pub gas_limit: u64,
    /// Gas used by all transactions in this block.
    pub gas_used: u64,
    /// Base fee per gas (EIP-1559).
    pub base_fee_per_gas: Option<u64>,
    /// Hashes of transactions included in this block.
    pub transaction_hashes: Vec<B256>,
}

/// An indexed transaction with full details.
#[derive(Debug, Clone)]
pub struct IndexedTransaction {
    /// Transaction hash.
    pub hash: B256,
    /// Hash of the block containing this transaction.
    pub block_hash: B256,
    /// Number of the block containing this transaction.
    pub block_number: u64,
    /// Index of the transaction within the block.
    pub index: u64,
    /// Sender address.
    pub from: Address,
    /// Recipient address (None for contract creation).
    pub to: Option<Address>,
    /// Value transferred.
    pub value: U256,
    /// Gas limit for this transaction.
    pub gas_limit: u64,
    /// Gas price.
    pub gas_price: u128,
    /// Input data.
    pub input: Bytes,
    /// Sender nonce.
    pub nonce: u64,
}

/// An indexed transaction receipt.
#[derive(Debug, Clone)]
pub struct IndexedReceipt {
    /// Transaction hash.
    pub transaction_hash: B256,
    /// Hash of the block containing this transaction.
    pub block_hash: B256,
    /// Number of the block containing this transaction.
    pub block_number: u64,
    /// Index of the transaction within the block.
    pub transaction_index: u64,
    /// Sender address.
    pub from: Address,
    /// Recipient address (None for contract creation).
    pub to: Option<Address>,
    /// Cumulative gas used in the block up to and including this transaction.
    pub cumulative_gas_used: u64,
    /// Gas used by this transaction.
    pub gas_used: u64,
    /// Contract address created (if contract creation transaction).
    pub contract_address: Option<Address>,
    /// Logs emitted by this transaction.
    pub logs: Vec<IndexedLog>,
    /// Transaction status (true = success, false = revert).
    pub status: bool,
}

/// An indexed log entry.
#[derive(Debug, Clone)]
pub struct IndexedLog {
    /// Address of the contract that emitted the log.
    pub address: Address,
    /// Indexed topics.
    pub topics: Vec<B256>,
    /// Non-indexed data.
    pub data: Bytes,
    /// Log index within the block.
    pub log_index: u64,
}
