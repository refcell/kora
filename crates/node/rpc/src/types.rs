//! RPC types for Ethereum JSON-RPC API responses.

use alloy_primitives::{Address, B64, B256, Bytes, U64, U256};
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

/// Log filter for `eth_getLogs` queries.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcLogFilter {
    /// Start block (inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_block: Option<BlockNumberOrTag>,
    /// End block (inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_block: Option<BlockNumberOrTag>,
    /// Contract address or list of addresses to filter by.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<AddressFilter>,
    /// Topics to filter by. Each element is OR-matched within, AND-matched across positions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topics: Option<Vec<Option<TopicFilter>>>,
    /// Block hash to filter by (overrides from_block/to_block).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<B256>,
}

/// Address filter that can be a single address or list of addresses.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum AddressFilter {
    /// Single address.
    Single(Address),
    /// Multiple addresses (OR logic).
    Multiple(Vec<Address>),
}

impl AddressFilter {
    /// Converts to a vector of addresses.
    pub fn into_vec(self) -> Vec<Address> {
        match self {
            Self::Single(addr) => vec![addr],
            Self::Multiple(addrs) => addrs,
        }
    }
}

/// Topic filter that can be a single topic or list of topics.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TopicFilter {
    /// Single topic.
    Single(B256),
    /// Multiple topics (OR logic).
    Multiple(Vec<B256>),
}

impl TopicFilter {
    /// Converts to a vector of topics.
    pub fn into_vec(self) -> Vec<B256> {
        match self {
            Self::Single(topic) => vec![topic],
            Self::Multiple(topics) => topics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_number_or_tag_is_pending() {
        let pending = BlockNumberOrTag::Tag(BlockTag::Pending);
        let latest = BlockNumberOrTag::Tag(BlockTag::Latest);
        let number = BlockNumberOrTag::Number(U64::from(100));

        assert!(pending.is_pending());
        assert!(!latest.is_pending());
        assert!(!number.is_pending());
    }

    #[test]
    fn block_number_or_tag_is_latest() {
        let latest_tag = BlockNumberOrTag::Tag(BlockTag::Latest);
        let latest_default = BlockNumberOrTag::Latest;
        let pending = BlockNumberOrTag::Tag(BlockTag::Pending);
        let number = BlockNumberOrTag::Number(U64::from(100));

        assert!(latest_tag.is_latest());
        assert!(latest_default.is_latest());
        assert!(!pending.is_latest());
        assert!(!number.is_latest());
    }

    #[test]
    fn block_number_or_tag_default() {
        let default = BlockNumberOrTag::default();
        assert!(default.is_latest());
    }

    #[test]
    fn block_tag_default() {
        let default = BlockTag::default();
        assert_eq!(default, BlockTag::Latest);
    }

    #[test]
    fn block_transactions_default() {
        let default = BlockTransactions::default();
        assert!(matches!(default, BlockTransactions::Hashes(v) if v.is_empty()));
    }

    #[test]
    fn call_request_input_data_prefers_input() {
        let req = CallRequest {
            input: Some(Bytes::from_static(&[0x01, 0x02])),
            data: Some(Bytes::from_static(&[0x03, 0x04])),
            ..Default::default()
        };
        assert_eq!(req.input_data(), Bytes::from_static(&[0x01, 0x02]));
    }

    #[test]
    fn call_request_input_data_falls_back_to_data() {
        let req =
            CallRequest { data: Some(Bytes::from_static(&[0x03, 0x04])), ..Default::default() };
        assert_eq!(req.input_data(), Bytes::from_static(&[0x03, 0x04]));
    }

    #[test]
    fn call_request_input_data_returns_empty_if_none() {
        let req = CallRequest::default();
        assert!(req.input_data().is_empty());
    }

    #[test]
    fn block_tag_serde_roundtrip() {
        let tags = [
            BlockTag::Earliest,
            BlockTag::Finalized,
            BlockTag::Safe,
            BlockTag::Latest,
            BlockTag::Pending,
        ];
        for tag in tags {
            let json = serde_json::to_string(&tag).unwrap();
            let parsed: BlockTag = serde_json::from_str(&json).unwrap();
            assert_eq!(tag, parsed);
        }
    }

    #[test]
    fn block_number_or_tag_serde_number() {
        let block = BlockNumberOrTag::Number(U64::from(12345));
        let json = serde_json::to_string(&block).unwrap();
        let parsed: BlockNumberOrTag = serde_json::from_str(&json).unwrap();
        assert_eq!(block, parsed);
    }

    #[test]
    fn block_number_or_tag_serde_tag() {
        let block = BlockNumberOrTag::Tag(BlockTag::Finalized);
        let json = serde_json::to_string(&block).unwrap();
        let parsed: BlockNumberOrTag = serde_json::from_str(&json).unwrap();
        assert_eq!(block, parsed);
    }

    #[test]
    fn rpc_block_default() {
        let block = RpcBlock::default();
        assert_eq!(block.hash, B256::ZERO);
        assert_eq!(block.number, U64::ZERO);
    }

    #[test]
    fn rpc_transaction_default() {
        let tx = RpcTransaction::default();
        assert_eq!(tx.hash, B256::ZERO);
        assert_eq!(tx.from, Address::ZERO);
    }

    #[test]
    fn rpc_transaction_receipt_default() {
        let receipt = RpcTransactionReceipt::default();
        assert_eq!(receipt.transaction_hash, B256::ZERO);
        assert!(receipt.logs.is_empty());
    }

    #[test]
    fn rpc_log_default() {
        let log = RpcLog::default();
        assert_eq!(log.address, Address::ZERO);
        assert!(log.topics.is_empty());
        assert!(!log.removed);
    }

    #[test]
    fn sync_status_not_syncing() {
        let status = SyncStatus::NotSyncing(false);
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "false");
    }

    #[test]
    fn sync_status_syncing() {
        let info = SyncInfo {
            starting_block: U64::from(0),
            current_block: U64::from(100),
            highest_block: U64::from(200),
        };
        let status = SyncStatus::Syncing(info);
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("startingBlock"));
        assert!(json.contains("currentBlock"));
        assert!(json.contains("highestBlock"));
    }

    #[test]
    fn rpc_log_filter_default() {
        let filter = RpcLogFilter::default();
        assert!(filter.from_block.is_none());
        assert!(filter.to_block.is_none());
        assert!(filter.address.is_none());
        assert!(filter.topics.is_none());
        assert!(filter.block_hash.is_none());
    }

    #[test]
    fn address_filter_single_into_vec() {
        let addr = Address::repeat_byte(0x42);
        let filter = AddressFilter::Single(addr);
        let addrs = filter.into_vec();
        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0], addr);
    }

    #[test]
    fn address_filter_multiple_into_vec() {
        let addr1 = Address::repeat_byte(0x01);
        let addr2 = Address::repeat_byte(0x02);
        let filter = AddressFilter::Multiple(vec![addr1, addr2]);
        let addrs = filter.into_vec();
        assert_eq!(addrs.len(), 2);
    }

    #[test]
    fn topic_filter_single_into_vec() {
        let topic = B256::repeat_byte(0xab);
        let filter = TopicFilter::Single(topic);
        let topics = filter.into_vec();
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0], topic);
    }

    #[test]
    fn topic_filter_multiple_into_vec() {
        let topic1 = B256::repeat_byte(0x01);
        let topic2 = B256::repeat_byte(0x02);
        let filter = TopicFilter::Multiple(vec![topic1, topic2]);
        let topics = filter.into_vec();
        assert_eq!(topics.len(), 2);
    }

    #[test]
    fn address_filter_serde_single() {
        let addr = Address::repeat_byte(0x42);
        let json = format!("\"{addr}\"");
        let filter: AddressFilter = serde_json::from_str(&json).unwrap();
        assert!(matches!(filter, AddressFilter::Single(a) if a == addr));
    }

    #[test]
    fn address_filter_serde_multiple() {
        let addr1 = Address::repeat_byte(0x01);
        let addr2 = Address::repeat_byte(0x02);
        let json = format!("[\"{addr1}\", \"{addr2}\"]");
        let filter: AddressFilter = serde_json::from_str(&json).unwrap();
        assert!(matches!(filter, AddressFilter::Multiple(addrs) if addrs.len() == 2));
    }
}
