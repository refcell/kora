//! State provider trait for abstracting state access in RPC methods.

use alloy_primitives::{Address, Bytes, B256, U256};
use async_trait::async_trait;

use crate::{
    error::RpcError,
    types::{BlockNumberOrTag, CallRequest, RpcBlock, RpcTransaction, RpcTransactionReceipt},
};

/// Trait for providing state access to RPC methods.
///
/// This abstracts away the underlying storage implementation, allowing
/// RPC methods to query state without knowing about ledger internals.
#[async_trait]
pub trait StateProvider: Send + Sync {
    /// Get the balance of an account at a given block.
    async fn balance(&self, address: Address, block: Option<BlockNumberOrTag>)
        -> Result<U256, RpcError>;

    /// Get the nonce (transaction count) of an account at a given block.
    async fn nonce(&self, address: Address, block: Option<BlockNumberOrTag>)
        -> Result<u64, RpcError>;

    /// Get the code of an account at a given block.
    async fn code(&self, address: Address, block: Option<BlockNumberOrTag>)
        -> Result<Bytes, RpcError>;

    /// Get a storage slot value at a given block.
    async fn storage(
        &self,
        address: Address,
        slot: U256,
        block: Option<BlockNumberOrTag>,
    ) -> Result<U256, RpcError>;

    /// Get a block by number.
    async fn block_by_number(&self, block: BlockNumberOrTag) -> Result<Option<RpcBlock>, RpcError>;

    /// Get a block by hash.
    async fn block_by_hash(&self, hash: B256) -> Result<Option<RpcBlock>, RpcError>;

    /// Get a transaction by hash.
    async fn transaction_by_hash(&self, hash: B256) -> Result<Option<RpcTransaction>, RpcError>;

    /// Get a transaction receipt by hash.
    async fn receipt_by_hash(&self, hash: B256) -> Result<Option<RpcTransactionReceipt>, RpcError>;

    /// Get the current block number.
    async fn block_number(&self) -> Result<u64, RpcError>;

    /// Execute a call without creating a transaction.
    async fn call(
        &self,
        _request: CallRequest,
        _block: Option<BlockNumberOrTag>,
    ) -> Result<Bytes, RpcError> {
        Err(RpcError::NotImplemented)
    }

    /// Estimate gas for a transaction.
    async fn estimate_gas(
        &self,
        _request: CallRequest,
        _block: Option<BlockNumberOrTag>,
    ) -> Result<u64, RpcError> {
        Err(RpcError::NotImplemented)
    }
}

/// A no-op state provider that returns empty/zero values.
///
/// Useful for testing or when state access is not yet implemented.
#[derive(Clone, Debug, Default)]
pub struct NoopStateProvider;

#[async_trait]
impl StateProvider for NoopStateProvider {
    async fn balance(
        &self,
        _address: Address,
        _block: Option<BlockNumberOrTag>,
    ) -> Result<U256, RpcError> {
        Ok(U256::ZERO)
    }

    async fn nonce(
        &self,
        _address: Address,
        _block: Option<BlockNumberOrTag>,
    ) -> Result<u64, RpcError> {
        Ok(0)
    }

    async fn code(
        &self,
        _address: Address,
        _block: Option<BlockNumberOrTag>,
    ) -> Result<Bytes, RpcError> {
        Ok(Bytes::new())
    }

    async fn storage(
        &self,
        _address: Address,
        _slot: U256,
        _block: Option<BlockNumberOrTag>,
    ) -> Result<U256, RpcError> {
        Ok(U256::ZERO)
    }

    async fn block_by_number(&self, _block: BlockNumberOrTag) -> Result<Option<RpcBlock>, RpcError> {
        Ok(None)
    }

    async fn block_by_hash(&self, _hash: B256) -> Result<Option<RpcBlock>, RpcError> {
        Ok(None)
    }

    async fn transaction_by_hash(&self, _hash: B256) -> Result<Option<RpcTransaction>, RpcError> {
        Ok(None)
    }

    async fn receipt_by_hash(&self, _hash: B256) -> Result<Option<RpcTransactionReceipt>, RpcError> {
        Ok(None)
    }

    async fn block_number(&self) -> Result<u64, RpcError> {
        Err(RpcError::NotImplemented)
    }
}
