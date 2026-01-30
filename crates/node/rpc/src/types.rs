//! RPC types for Ethereum JSON-RPC API responses.

use alloy_primitives::{Address, Bytes, B256, B64, U256, U64};
use serde::{Deserialize, Serialize};

/// Block number or tag for RPC queries.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum BlockNumberOrTag {
    /// Block number.
    Number(U64),
    /// Block tag.
    Tag(BlockTag),
    /// Default to latest.
    #[default]
    #[serde(skip)]
    Latest,
}

/// Block tags for RPC queries.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BlockTag {
    /// Earliest block (genesis).
    Earliest,
    /// Finalized block.
    Finalized,
    /// Safe block.
    Safe,
    /// Latest block.
    #[default]
    Latest,
    /// Pending block.
    Pending,
}

impl BlockNumberOrTag {
    /// Returns true if this is a pending block reference.
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Tag(BlockTag::Pending))
    }

    /// Returns true if this is the latest block reference.
    pub const fn is_latest(&self) -> bool {
        matches!(self, Self::Tag(BlockTag::Latest) | Self::Latest)
    }
}

/// Rich block representation for JSON-RPC responses.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcBlock {
    /// Block hash.
    pub hash: B256,
    /// Parent block hash.
    pub parent_hash: B256,
    /// Block number.
    pub number: U64,
    /// State root.
    pub state_root: B256,
    /// Transactions root.
    pub transactions_root: B256,
    /// Receipts root.
    pub receipts_root: B256,
    /// Logs bloom filter.
    pub logs_bloom: Bytes,
    /// Block timestamp.
    pub timestamp: U64,
    /// Gas limit.
    pub gas_limit: U64,
    /// Gas used.
    pub gas_used: U64,
    /// Extra data.
    pub extra_data: Bytes,
    /// Mix hash (prevrandao).
    pub mix_hash: B256,
    /// Nonce.
    pub nonce: B64,
    /// Base fee per gas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_fee_per_gas: Option<U256>,
    /// Miner/beneficiary address.
    pub miner: Address,
    /// Difficulty (always 0 post-merge).
    pub difficulty: U256,
    /// Total difficulty (always 0 post-merge).
    pub total_difficulty: U256,
    /// Uncles (always empty post-merge).
    pub uncles: Vec<B256>,
    /// Block size.
    pub size: U64,
    /// Transactions (hashes or full objects).
    pub transactions: BlockTransactions,
}

/// Transactions in a block response.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockTransactions {
    /// Only transaction hashes.
    Hashes(Vec<B256>),
    /// Full transaction objects.
    Full(Vec<RpcTransaction>),
}

impl Default for BlockTransactions {
    fn default() -> Self {
        Self::Hashes(Vec::new())
    }
}

/// Transaction object for JSON-RPC responses.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcTransaction {
    /// Transaction hash.
    pub hash: B256,
    /// Nonce.
    pub nonce: U64,
    /// Block hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<B256>,
    /// Block number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_number: Option<U64>,
    /// Transaction index in block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_index: Option<U64>,
    /// Sender address.
    pub from: Address,
    /// Recipient address (None for contract creation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<Address>,
    /// Value transferred.
    pub value: U256,
    /// Gas limit.
    pub gas: U64,
    /// Gas price.
    pub gas_price: U256,
    /// Input data.
    pub input: Bytes,
    /// Transaction type.
    #[serde(rename = "type")]
    pub tx_type: U64,
    /// Chain ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<U64>,
    /// Max fee per gas (EIP-1559).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_fee_per_gas: Option<U256>,
    /// Max priority fee per gas (EIP-1559).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_priority_fee_per_gas: Option<U256>,
    /// V component of signature.
    pub v: U64,
    /// R component of signature.
    pub r: U256,
    /// S component of signature.
    pub s: U256,
}

/// Transaction receipt for JSON-RPC responses.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcTransactionReceipt {
    /// Transaction hash.
    pub transaction_hash: B256,
    /// Transaction index in block.
    pub transaction_index: U64,
    /// Block hash.
    pub block_hash: B256,
    /// Block number.
    pub block_number: U64,
    /// Sender address.
    pub from: Address,
    /// Recipient address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<Address>,
    /// Cumulative gas used.
    pub cumulative_gas_used: U64,
    /// Gas used by this transaction.
    pub gas_used: U64,
    /// Contract address created (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_address: Option<Address>,
    /// Logs generated.
    pub logs: Vec<RpcLog>,
    /// Logs bloom filter.
    pub logs_bloom: Bytes,
    /// Transaction type.
    #[serde(rename = "type")]
    pub tx_type: U64,
    /// Status (1 = success, 0 = failure).
    pub status: U64,
    /// Effective gas price.
    pub effective_gas_price: U256,
}

/// Log entry for JSON-RPC responses.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcLog {
    /// Contract address.
    pub address: Address,
    /// Log topics.
    pub topics: Vec<B256>,
    /// Log data.
    pub data: Bytes,
    /// Block number.
    pub block_number: U64,
    /// Transaction hash.
    pub transaction_hash: B256,
    /// Transaction index.
    pub transaction_index: U64,
    /// Block hash.
    pub block_hash: B256,
    /// Log index in block.
    pub log_index: U64,
    /// Whether this log was removed due to reorg.
    pub removed: bool,
}

/// Call request for eth_call and eth_estimateGas.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallRequest {
    /// Sender address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<Address>,
    /// Recipient address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<Address>,
    /// Gas limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<U64>,
    /// Gas price.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<U256>,
    /// Max fee per gas (EIP-1559).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_fee_per_gas: Option<U256>,
    /// Max priority fee per gas (EIP-1559).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_priority_fee_per_gas: Option<U256>,
    /// Value to transfer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<U256>,
    /// Input data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Bytes>,
    /// Legacy data field (alias for input).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Bytes>,
    /// Nonce.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<U64>,
    /// Chain ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<U64>,
}

impl CallRequest {
    /// Get the input data, preferring `input` over `data`.
    pub fn input_data(&self) -> Bytes {
        self.input.clone().or_else(|| self.data.clone()).unwrap_or_default()
    }
}

/// Sync status for eth_syncing.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SyncStatus {
    /// Not syncing.
    NotSyncing(bool),
    /// Syncing status.
    Syncing(SyncInfo),
}

/// Syncing information.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncInfo {
    /// Starting block.
    pub starting_block: U64,
    /// Current block.
    pub current_block: U64,
    /// Highest block.
    pub highest_block: U64,
}
