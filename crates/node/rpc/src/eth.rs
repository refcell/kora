//! Ethereum JSON-RPC API implementation.

use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use alloy_consensus::{Transaction as _, TxEnvelope, transaction::SignerRecoverable as _};
use alloy_eips::eip2718::Decodable2718 as _;
use alloy_primitives::{Address, B256, Bytes, U64, U256};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use tokio::sync::RwLock;

use crate::{
    error::RpcError,
    state_provider::StateProvider,
    types::{
        BlockNumberOrTag, CallRequest, RpcBlock, RpcLog, RpcLogFilter, RpcTransaction,
        RpcTransactionReceipt,
    },
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
    async fn get_balance(
        &self,
        address: Address,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<U256>;

    /// Returns the nonce (transaction count) of an account.
    #[method(name = "getTransactionCount")]
    async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<U64>;

    /// Returns the code at an address.
    #[method(name = "getCode")]
    async fn get_code(&self, address: Address, block: Option<BlockNumberOrTag>)
    -> RpcResult<Bytes>;

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
    async fn call(&self, request: CallRequest, block: Option<BlockNumberOrTag>)
    -> RpcResult<Bytes>;

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
    async fn get_transaction_receipt(&self, hash: B256)
    -> RpcResult<Option<RpcTransactionReceipt>>;

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

    /// Returns logs matching the given filter.
    #[method(name = "getLogs")]
    async fn get_logs(&self, filter: RpcLogFilter) -> RpcResult<Vec<RpcLog>>;
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
/// Resolves successfully only if the transaction was accepted.
pub type TxSubmitFuture = Pin<Box<dyn Future<Output = Result<(), RpcError>> + Send>>;

/// Async transaction submission callback type.
pub type TxSubmitCallback = Arc<dyn Fn(Bytes) -> TxSubmitFuture + Send + Sync>;

/// Ethereum API implementation with state provider.
pub struct EthApiImpl<S: StateProvider> {
    chain_id: u64,
    block_height: Arc<std::sync::atomic::AtomicU64>,
    tx_submit: Option<TxSubmitCallback>,
    state_provider: Arc<RwLock<S>>,
    pending_txs: Arc<RwLock<HashMap<B256, RpcTransaction>>>,
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
            pending_txs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new Ethereum API implementation with a transaction submission callback.
    pub fn with_tx_submit(chain_id: u64, state_provider: S, tx_submit: TxSubmitCallback) -> Self {
        Self {
            chain_id,
            block_height: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            tx_submit: Some(tx_submit),
            state_provider: Arc::new(RwLock::new(state_provider)),
            pending_txs: Arc::new(RwLock::new(HashMap::new())),
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
        provider.block_number().await.map_or_else(
            |_| {
                let height = self.block_height.load(std::sync::atomic::Ordering::Relaxed);
                Ok(U64::from(height))
            },
            |height| Ok(U64::from(height)),
        )
    }

    async fn get_balance(
        &self,
        address: Address,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<U256> {
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

    async fn get_code(
        &self,
        address: Address,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<Bytes> {
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
        let pending_tx = raw_tx_to_pending_rpc(&data)?;

        if let Some(ref submit) = self.tx_submit {
            submit(data).await?;
        }

        self.pending_txs.write().await.insert(tx_hash, pending_tx);
        Ok(tx_hash)
    }

    async fn call(
        &self,
        request: CallRequest,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<Bytes> {
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
        full_transactions: bool,
    ) -> RpcResult<Option<RpcBlock>> {
        let provider = self.state_provider.read().await;
        provider.block_by_number(block, full_transactions).await.map_err(Into::into)
    }

    async fn get_block_by_hash(
        &self,
        hash: B256,
        full_transactions: bool,
    ) -> RpcResult<Option<RpcBlock>> {
        let provider = self.state_provider.read().await;
        provider.block_by_hash(hash, full_transactions).await.map_err(Into::into)
    }

    async fn get_transaction_by_hash(&self, hash: B256) -> RpcResult<Option<RpcTransaction>> {
        let provider = self.state_provider.read().await;
        let indexed = provider.transaction_by_hash(hash).await?;
        if indexed.is_some() {
            self.pending_txs.write().await.remove(&hash);
            return Ok(indexed);
        }
        Ok(self.pending_txs.read().await.get(&hash).cloned())
    }

    async fn get_transaction_receipt(
        &self,
        hash: B256,
    ) -> RpcResult<Option<RpcTransactionReceipt>> {
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
        newest_block: BlockNumberOrTag,
        reward_percentiles: Option<Vec<f64>>,
    ) -> RpcResult<FeeHistory> {
        let provider = self.state_provider.read().await;
        let head = provider
            .block_number()
            .await
            .unwrap_or_else(|_| self.block_height.load(std::sync::atomic::Ordering::Relaxed));
        let newest = match newest_block {
            BlockNumberOrTag::Number(n) => n.to::<u64>().min(head),
            BlockNumberOrTag::Tag(_) | BlockNumberOrTag::Latest => head,
        };
        let requested = block_count.to::<u64>().min(1024);
        let count = requested.min(newest.saturating_add(1)) as usize;
        let oldest = newest.saturating_add(1).saturating_sub(count as u64);
        let base_fee = U256::from(1_000_000_000u64);

        Ok(FeeHistory {
            base_fee_per_gas: vec![base_fee; count + 1],
            gas_used_ratio: vec![0.0; count],
            oldest_block: U64::from(oldest),
            reward: reward_percentiles.map(|percentiles| {
                vec![vec![U256::from(1_000_000_000u64); percentiles.len()]; count]
            }),
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

    async fn get_logs(&self, filter: RpcLogFilter) -> RpcResult<Vec<RpcLog>> {
        let provider = self.state_provider.read().await;
        provider.get_logs(filter).await.map_err(Into::into)
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

fn raw_tx_to_pending_rpc(data: &Bytes) -> Result<RpcTransaction, RpcError> {
    let envelope = TxEnvelope::decode_2718(&mut data.as_ref())
        .map_err(|err| RpcError::InvalidTransaction(format!("failed to decode: {err}")))?;
    let from = envelope
        .recover_signer()
        .map_err(|err| RpcError::InvalidTransaction(format!("failed to recover signer: {err}")))?;
    let signature = envelope.signature();
    let hash = alloy_primitives::keccak256(data);

    Ok(RpcTransaction {
        hash,
        nonce: U64::from(envelope.nonce()),
        block_hash: None,
        block_number: None,
        transaction_index: None,
        from,
        to: envelope.to(),
        value: envelope.value(),
        gas: U64::from(envelope.gas_limit()),
        gas_price: U256::from(effective_gas_price(&envelope)),
        input: envelope.input().clone(),
        tx_type: U64::from(transaction_type(&envelope)),
        chain_id: envelope.chain_id().map(U64::from),
        max_fee_per_gas: max_fee_per_gas(&envelope).map(U256::from),
        max_priority_fee_per_gas: max_priority_fee_per_gas(&envelope).map(U256::from),
        v: U64::from(u64::from(signature.v())),
        r: signature.r(),
        s: signature.s(),
    })
}

fn transaction_type(envelope: &TxEnvelope) -> u64 {
    match envelope {
        TxEnvelope::Legacy(_) => 0,
        TxEnvelope::Eip2930(_) => 1,
        TxEnvelope::Eip1559(_) => 2,
        TxEnvelope::Eip4844(_) => 3,
        TxEnvelope::Eip7702(_) => 4,
    }
}

fn effective_gas_price(envelope: &TxEnvelope) -> u128 {
    match envelope {
        TxEnvelope::Legacy(tx) => tx.tx().gas_price,
        TxEnvelope::Eip2930(tx) => tx.tx().gas_price,
        TxEnvelope::Eip1559(tx) => tx.tx().max_fee_per_gas,
        TxEnvelope::Eip4844(tx) => tx.tx().tx().max_fee_per_gas,
        TxEnvelope::Eip7702(tx) => tx.tx().max_fee_per_gas,
    }
}

fn max_fee_per_gas(envelope: &TxEnvelope) -> Option<u128> {
    match envelope {
        TxEnvelope::Legacy(_) | TxEnvelope::Eip2930(_) => None,
        TxEnvelope::Eip1559(tx) => Some(tx.tx().max_fee_per_gas),
        TxEnvelope::Eip4844(tx) => Some(tx.tx().tx().max_fee_per_gas),
        TxEnvelope::Eip7702(tx) => Some(tx.tx().max_fee_per_gas),
    }
}

fn max_priority_fee_per_gas(envelope: &TxEnvelope) -> Option<u128> {
    match envelope {
        TxEnvelope::Legacy(_) | TxEnvelope::Eip2930(_) => None,
        TxEnvelope::Eip1559(tx) => Some(tx.tx().max_priority_fee_per_gas),
        TxEnvelope::Eip4844(tx) => Some(tx.tx().tx().max_priority_fee_per_gas),
        TxEnvelope::Eip7702(tx) => Some(tx.tx().max_priority_fee_per_gas),
    }
}

#[cfg(test)]
mod tests {
    use alloy_consensus::{SignableTransaction as _, TxEip1559};
    use alloy_eips::eip2718::Encodable2718 as _;
    use alloy_primitives::{Signature, TxKind};
    use k256::ecdsa::SigningKey;
    use sha3::{Digest as _, Keccak256};

    use super::*;
    use crate::state_provider::NoopStateProvider;

    fn signed_test_tx(chain_id: u64, nonce: u64) -> Bytes {
        let mut secret = [0u8; 32];
        secret[31] = 1;
        let key = SigningKey::from_bytes((&secret).into()).expect("valid key");
        let tx = TxEip1559 {
            chain_id,
            nonce,
            gas_limit: 21_000,
            max_fee_per_gas: 1,
            max_priority_fee_per_gas: 1,
            to: TxKind::Call(Address::repeat_byte(0xbb)),
            value: U256::from(1),
            access_list: Default::default(),
            input: Bytes::new(),
        };
        let digest = Keccak256::new_with_prefix(tx.encoded_for_signing());
        let (sig, recid) = key.sign_digest_recoverable(digest).expect("sign tx");
        let signature = Signature::from((sig, recid));
        let envelope = TxEnvelope::from(tx.into_signed(signature));
        let mut raw = Vec::new();
        envelope.encode_2718(&mut raw);
        Bytes::from(raw)
    }

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
            Box::pin(async { Ok(()) })
        });

        let api = EthApiImpl::with_tx_submit(1, NoopStateProvider, callback);
        let tx_data = signed_test_tx(1, 0);
        let result = EthApiServer::send_raw_transaction(&api, tx_data.clone()).await;

        assert!(result.is_ok());
        assert!(submitted.load(std::sync::atomic::Ordering::Relaxed));
        assert_eq!(result.unwrap(), alloy_primitives::keccak256(&tx_data));
    }

    #[tokio::test]
    async fn eth_get_transaction_by_hash_returns_pending_submission() {
        let callback: TxSubmitCallback = Arc::new(move |_| Box::pin(async { Ok(()) }));
        let api = EthApiImpl::with_tx_submit(1, NoopStateProvider, callback);
        let tx_data = signed_test_tx(1, 7);
        let hash = EthApiServer::send_raw_transaction(&api, tx_data).await.unwrap();

        let tx = EthApiServer::get_transaction_by_hash(&api, hash).await.unwrap();
        let tx = tx.expect("pending transaction should be visible");
        assert_eq!(tx.hash, hash);
        assert_eq!(tx.nonce, U64::from(7));
        assert!(tx.block_hash.is_none());
    }

    /// Regression: when no `tx_submit` callback is wired, `send_raw_transaction`
    /// silently accepts the tx and returns the hash, but the tx goes nowhere —
    /// no mempool, no producer, no block. This is exactly the failure mode
    /// observed on devnet 1337 (`http://65.109.61.210:8545`) where the deployed
    /// kora binary predates the runner's `with_tx_submit(...)` wiring (commit
    /// `beb637a`): every tx submitted via JSON-RPC was accepted, hash returned,
    /// but never included in any block.
    ///
    /// The fix lives in the runner: always wire `tx_submit` to a real mempool.
    /// The downstream observability fix (warn-log when build_block produces an
    /// empty block while the mempool is non-empty) lives in `app.rs`.
    #[tokio::test]
    async fn send_raw_transaction_with_no_callback_silently_accepts_but_drops() {
        let api = EthApiImpl::new(1, NoopStateProvider); // no tx_submit
        let tx_data = signed_test_tx(1, 0);
        let result = EthApiServer::send_raw_transaction(&api, tx_data.clone()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), alloy_primitives::keccak256(&tx_data));
        // The tx is in pending_txs (so getTransactionByHash returns something) —
        // that's exactly what makes the bug invisible to operators.
        let cached =
            EthApiServer::get_transaction_by_hash(&api, alloy_primitives::keccak256(&tx_data))
                .await
                .unwrap();
        assert!(
            cached.is_some(),
            "RPC caches the tx for visibility even though it has nowhere to send it"
        );
    }

    /// Regression: the existing `eth_send_raw_transaction` test only verifies
    /// that the callback is invoked (a boolean flag). It does not verify that
    /// the bytes passed to the callback are the same bytes the caller sent.
    /// A regression that mangled the body (e.g. dropped the chainId, re-encoded
    /// the envelope, sent a partial slice) would still pass that test. This
    /// one captures the actual bytes and compares them.
    #[tokio::test]
    async fn send_raw_transaction_passes_full_tx_bytes_to_callback() {
        let captured: Arc<RwLock<Vec<Bytes>>> = Arc::new(RwLock::new(Vec::new()));
        let captured_clone = captured.clone();
        let callback: TxSubmitCallback = Arc::new(move |data| {
            let captured_clone = captured_clone.clone();
            Box::pin(async move {
                captured_clone.write().await.push(data);
                Ok(())
            })
        });
        let api = EthApiImpl::with_tx_submit(1, NoopStateProvider, callback);
        let tx_data = signed_test_tx(1, 42);
        let _ = EthApiServer::send_raw_transaction(&api, tx_data.clone()).await.unwrap();
        let inner = captured.read().await;
        assert_eq!(inner.len(), 1, "callback invoked exactly once");
        assert_eq!(
            &inner[0][..],
            &tx_data[..],
            "callback receives the caller's tx bytes verbatim — no re-encoding, no truncation"
        );
    }
}
