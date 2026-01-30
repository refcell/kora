//! Ethereum JSON-RPC API implementation.

use std::sync::Arc;

use alloy_primitives::{Address, Bytes, B256, U256, U64};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use tokio::sync::RwLock;

use crate::{
    error::RpcError,
    state_provider::StateProvider,
    types::{BlockNumberOrTag, CallRequest, RpcBlock, RpcTransaction, RpcTransactionReceipt},
};

/// Ethereum JSON-RPC API trait.
///
/// Defines the core eth_* methods required for Ethereum compatibility.
#[rpc(server, namespace = "eth")]
pub trait EthApi {
    /// Returns the chain ID.
    #[method(name = "chainId")]
    async fn chain_id(&self) -> RpcResult<U64>;

    /// Returns the current block number.
    #[method(name = "blockNumber")]
    async fn block_number(&self) -> RpcResult<U64>;

    /// Returns the balance of an account.
    #[method(name = "getBalance")]
    async fn get_balance(&self, address: Address, block: Option<BlockNumberOrTag>) -> RpcResult<U256>;

    /// Returns the nonce (transaction count) of an account.
    #[method(name = "getTransactionCount")]
    async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<U64>;

    /// Returns the code at an address.
    #[method(name = "getCode")]
    async fn get_code(&self, address: Address, block: Option<BlockNumberOrTag>) -> RpcResult<Bytes>;

    /// Returns the value of a storage slot.
    #[method(name = "getStorageAt")]
    async fn get_storage_at(
        &self,
        address: Address,
        slot: U256,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<U256>;

    /// Submits a raw transaction to the mempool.
    #[method(name = "sendRawTransaction")]
    async fn send_raw_transaction(&self, data: Bytes) -> RpcResult<B256>;

    /// Executes a call without creating a transaction.
    #[method(name = "call")]
    async fn call(&self, request: CallRequest, block: Option<BlockNumberOrTag>) -> RpcResult<Bytes>;

    /// Estimates gas for a transaction.
    #[method(name = "estimateGas")]
    async fn estimate_gas(
        &self,
        request: CallRequest,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<U64>;

    /// Returns a block by number.
    #[method(name = "getBlockByNumber")]
    async fn get_block_by_number(
        &self,
        block: BlockNumberOrTag,
        full_transactions: bool,
    ) -> RpcResult<Option<RpcBlock>>;

    /// Returns a block by hash.
    #[method(name = "getBlockByHash")]
    async fn get_block_by_hash(
        &self,
        hash: B256,
        full_transactions: bool,
    ) -> RpcResult<Option<RpcBlock>>;

    /// Returns a transaction by hash.
    #[method(name = "getTransactionByHash")]
    async fn get_transaction_by_hash(&self, hash: B256) -> RpcResult<Option<RpcTransaction>>;

    /// Returns a transaction receipt by hash.
    #[method(name = "getTransactionReceipt")]
    async fn get_transaction_receipt(&self, hash: B256) -> RpcResult<Option<RpcTransactionReceipt>>;

    /// Returns the current gas price.
    #[method(name = "gasPrice")]
    async fn gas_price(&self) -> RpcResult<U256>;

    /// Returns the max priority fee per gas.
    #[method(name = "maxPriorityFeePerGas")]
    async fn max_priority_fee_per_gas(&self) -> RpcResult<U256>;

    /// Returns fee history.
    #[method(name = "feeHistory")]
    async fn fee_history(
        &self,
        block_count: U64,
        newest_block: BlockNumberOrTag,
        reward_percentiles: Option<Vec<f64>>,
    ) -> RpcResult<FeeHistory>;

    /// Returns the accounts owned by the client (empty for non-wallet nodes).
    #[method(name = "accounts")]
    async fn accounts(&self) -> RpcResult<Vec<Address>>;

    /// Returns protocol version.
    #[method(name = "protocolVersion")]
    async fn protocol_version(&self) -> RpcResult<String>;

    /// Returns syncing status.
    #[method(name = "syncing")]
    async fn syncing(&self) -> RpcResult<bool>;
}

/// Net namespace API.
#[rpc(server, namespace = "net")]
pub trait NetApi {
    /// Returns the network ID.
    #[method(name = "version")]
    fn version(&self) -> RpcResult<String>;

    /// Returns true if the client is listening for connections.
    #[method(name = "listening")]
    fn listening(&self) -> RpcResult<bool>;

    /// Returns the number of connected peers.
    #[method(name = "peerCount")]
    fn peer_count(&self) -> RpcResult<U64>;
}

/// Web3 namespace API.
#[rpc(server, namespace = "web3")]
pub trait Web3Api {
    /// Returns the client version.
    #[method(name = "clientVersion")]
    fn client_version(&self) -> RpcResult<String>;

    /// Returns the Keccak-256 hash of the given data.
    #[method(name = "sha3")]
    fn sha3(&self, data: Bytes) -> RpcResult<B256>;
}

/// Fee history response.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeHistory {
    /// Base fee per gas for each block.
    pub base_fee_per_gas: Vec<U256>,
    /// Gas used ratio for each block.
    pub gas_used_ratio: Vec<f64>,
    /// Oldest block number.
    pub oldest_block: U64,
    /// Reward percentiles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reward: Option<Vec<Vec<U256>>>,
}

/// Transaction submission callback type.
///
/// Called when a raw transaction is submitted via `eth_sendRawTransaction`.
/// Returns true if the transaction was accepted, false otherwise.
pub type TxSubmitCallback = Arc<dyn Fn(Bytes) -> bool + Send + Sync>;

/// Ethereum API implementation with state provider.
pub struct EthApiImpl<S: StateProvider> {
    chain_id: u64,
    block_height: Arc<std::sync::atomic::AtomicU64>,
    tx_submit: Option<TxSubmitCallback>,
    state_provider: Arc<RwLock<S>>,
}

impl<S: StateProvider> std::fmt::Debug for EthApiImpl<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EthApiImpl")
            .field("chain_id", &self.chain_id)
            .field("block_height", &self.block_height)
            .field("tx_submit", &self.tx_submit.is_some())
            .finish()
    }
}

impl<S: StateProvider + 'static> EthApiImpl<S> {
    /// Create a new Ethereum API implementation with a state provider.
    pub fn new(chain_id: u64, state_provider: S) -> Self {
        Self {
            chain_id,
            block_height: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            tx_submit: None,
            state_provider: Arc::new(RwLock::new(state_provider)),
        }
    }

    /// Create a new Ethereum API implementation with a transaction submission callback.
    pub fn with_tx_submit(chain_id: u64, state_provider: S, tx_submit: TxSubmitCallback) -> Self {
        Self {
            chain_id,
            block_height: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            tx_submit: Some(tx_submit),
            state_provider: Arc::new(RwLock::new(state_provider)),
        }
    }

    /// Get a handle to update the block height.
    pub fn block_height_handle(&self) -> Arc<std::sync::atomic::AtomicU64> {
        self.block_height.clone()
    }

    /// Update the current block height.
    pub fn set_block_height(&self, height: u64) {
        self.block_height.store(height, std::sync::atomic::Ordering::Relaxed);
    }
}

#[jsonrpsee::core::async_trait]
impl<S: StateProvider + 'static> EthApiServer for EthApiImpl<S> {
    async fn chain_id(&self) -> RpcResult<U64> {
        Ok(U64::from(self.chain_id))
    }

    async fn block_number(&self) -> RpcResult<U64> {
        let provider = self.state_provider.read().await;
        match provider.block_number().await {
            Ok(height) => Ok(U64::from(height)),
            Err(_) => {
                let height = self.block_height.load(std::sync::atomic::Ordering::Relaxed);
                Ok(U64::from(height))
            }
        }
    }

    async fn get_balance(&self, address: Address, block: Option<BlockNumberOrTag>) -> RpcResult<U256> {
        let provider = self.state_provider.read().await;
        provider.balance(address, block).await.map_err(Into::into)
    }

    async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<U64> {
        let provider = self.state_provider.read().await;
        let nonce = provider.nonce(address, block).await?;
        Ok(U64::from(nonce))
    }

    async fn get_code(&self, address: Address, block: Option<BlockNumberOrTag>) -> RpcResult<Bytes> {
        let provider = self.state_provider.read().await;
        provider.code(address, block).await.map_err(Into::into)
    }

    async fn get_storage_at(
        &self,
        address: Address,
        slot: U256,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<U256> {
        let provider = self.state_provider.read().await;
        provider.storage(address, slot, block).await.map_err(Into::into)
    }

    async fn send_raw_transaction(&self, data: Bytes) -> RpcResult<B256> {
        let tx_hash = alloy_primitives::keccak256(&data);

        if let Some(ref submit) = self.tx_submit
            && !submit(data)
        {
            return Err(RpcError::InvalidTransaction("transaction rejected".into()).into());
        }

        Ok(tx_hash)
    }

    async fn call(&self, request: CallRequest, block: Option<BlockNumberOrTag>) -> RpcResult<Bytes> {
        let provider = self.state_provider.read().await;
        provider.call(request, block).await.map_err(Into::into)
    }

    async fn estimate_gas(
        &self,
        request: CallRequest,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<U64> {
        let provider = self.state_provider.read().await;
        let gas = provider.estimate_gas(request, block).await?;
        Ok(U64::from(gas))
    }

    async fn get_block_by_number(
        &self,
        block: BlockNumberOrTag,
        _full_transactions: bool,
    ) -> RpcResult<Option<RpcBlock>> {
        let provider = self.state_provider.read().await;
        provider.block_by_number(block).await.map_err(Into::into)
    }

    async fn get_block_by_hash(
        &self,
        hash: B256,
        _full_transactions: bool,
    ) -> RpcResult<Option<RpcBlock>> {
        let provider = self.state_provider.read().await;
        provider.block_by_hash(hash).await.map_err(Into::into)
    }

    async fn get_transaction_by_hash(&self, hash: B256) -> RpcResult<Option<RpcTransaction>> {
        let provider = self.state_provider.read().await;
        provider.transaction_by_hash(hash).await.map_err(Into::into)
    }

    async fn get_transaction_receipt(&self, hash: B256) -> RpcResult<Option<RpcTransactionReceipt>> {
        let provider = self.state_provider.read().await;
        provider.receipt_by_hash(hash).await.map_err(Into::into)
    }

    async fn gas_price(&self) -> RpcResult<U256> {
        Ok(U256::from(1_000_000_000u64))
    }

    async fn max_priority_fee_per_gas(&self) -> RpcResult<U256> {
        Ok(U256::from(1_000_000_000u64))
    }

    async fn fee_history(
        &self,
        block_count: U64,
        _newest_block: BlockNumberOrTag,
        reward_percentiles: Option<Vec<f64>>,
    ) -> RpcResult<FeeHistory> {
        let count = block_count.to::<usize>().min(1024);
        let base_fee = U256::from(1_000_000_000u64);

        Ok(FeeHistory {
            base_fee_per_gas: vec![base_fee; count + 1],
            gas_used_ratio: vec![0.5; count],
            oldest_block: U64::ZERO,
            reward: reward_percentiles
                .map(|percentiles| vec![vec![U256::from(1_000_000_000u64); percentiles.len()]; count]),
        })
    }

    async fn accounts(&self) -> RpcResult<Vec<Address>> {
        Ok(Vec::new())
    }

    async fn protocol_version(&self) -> RpcResult<String> {
        Ok("0x44".to_string())
    }

    async fn syncing(&self) -> RpcResult<bool> {
        Ok(false)
    }
}

/// Net API implementation.
pub struct NetApiImpl {
    chain_id: u64,
    peer_count: Arc<std::sync::atomic::AtomicU64>,
}

impl std::fmt::Debug for NetApiImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetApiImpl")
            .field("chain_id", &self.chain_id)
            .field("peer_count", &self.peer_count.load(std::sync::atomic::Ordering::Relaxed))
            .finish()
    }
}

impl NetApiImpl {
    /// Create a new Net API implementation.
    pub fn new(chain_id: u64) -> Self {
        Self { chain_id, peer_count: Arc::new(std::sync::atomic::AtomicU64::new(0)) }
    }

    /// Get a handle to update the peer count.
    pub fn peer_count_handle(&self) -> Arc<std::sync::atomic::AtomicU64> {
        self.peer_count.clone()
    }

    /// Update the peer count.
    pub fn set_peer_count(&self, count: u64) {
        self.peer_count.store(count, std::sync::atomic::Ordering::Relaxed);
    }
}

impl NetApiServer for NetApiImpl {
    fn version(&self) -> RpcResult<String> {
        Ok(self.chain_id.to_string())
    }

    fn listening(&self) -> RpcResult<bool> {
        Ok(true)
    }

    fn peer_count(&self) -> RpcResult<U64> {
        let count = self.peer_count.load(std::sync::atomic::Ordering::Relaxed);
        Ok(U64::from(count))
    }
}

/// Web3 API implementation.
#[derive(Clone, Debug, Default)]
pub struct Web3ApiImpl;

impl Web3ApiImpl {
    /// Create a new Web3 API implementation.
    pub const fn new() -> Self {
        Self
    }
}

impl Web3ApiServer for Web3ApiImpl {
    fn client_version(&self) -> RpcResult<String> {
        Ok(format!("kora/{}", env!("CARGO_PKG_VERSION")))
    }

    fn sha3(&self, data: Bytes) -> RpcResult<B256> {
        Ok(alloy_primitives::keccak256(&data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_provider::NoopStateProvider;

    #[test]
    fn web3_client_version() {
        let api = Web3ApiImpl::new();
        let version = Web3ApiServer::client_version(&api).unwrap();
        assert!(version.starts_with("kora/"));
    }

    #[tokio::test]
    async fn eth_chain_id() {
        let api = EthApiImpl::new(1337, NoopStateProvider);
        let chain_id = EthApiServer::chain_id(&api).await.unwrap();
        assert_eq!(chain_id, U64::from(1337));
    }

    #[test]
    fn net_version() {
        let api = NetApiImpl::new(1337);
        let version = NetApiServer::version(&api).unwrap();
        assert_eq!(version, "1337");
    }

    #[tokio::test]
    async fn eth_block_number() {
        let api = EthApiImpl::new(1, NoopStateProvider);
        api.set_block_height(42);
        let block_number = EthApiServer::block_number(&api).await.unwrap();
        assert_eq!(block_number, U64::from(42));
    }

    #[test]
    fn web3_sha3() {
        let api = Web3ApiImpl::new();
        let hash = Web3ApiServer::sha3(&api, Bytes::from_static(b"hello")).unwrap();
        assert_eq!(hash, alloy_primitives::keccak256(b"hello"));
    }

    #[tokio::test]
    async fn eth_send_raw_transaction() {
        let submitted = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let submitted_clone = submitted.clone();
        let callback: TxSubmitCallback = Arc::new(move |_| {
            submitted_clone.store(true, std::sync::atomic::Ordering::Relaxed);
            true
        });

        let api = EthApiImpl::with_tx_submit(1, NoopStateProvider, callback);
        let tx_data = Bytes::from_static(b"fake_tx_data");
        let result = EthApiServer::send_raw_transaction(&api, tx_data.clone()).await;

        assert!(result.is_ok());
        assert!(submitted.load(std::sync::atomic::Ordering::Relaxed));
        assert_eq!(result.unwrap(), alloy_primitives::keccak256(&tx_data));
    }
}
