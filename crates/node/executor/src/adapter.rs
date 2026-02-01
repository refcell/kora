//! State database adapter for REVM.
//!
//! Note: REVM's `DatabaseRef` trait is synchronous, so we use `futures::executor::block_on`
//! to bridge the async StateDb traits into the sync REVM interface.

use alloy_primitives::{Address, B256, KECCAK256_EMPTY, U256};
use kora_traits::{StateDbError, StateDbRead};
use revm::{bytecode::Bytecode, database_interface::DatabaseRef, state::AccountInfo};

use crate::ExecutionError;

/// Wrapper for blocking async operations in sync contexts.
fn block_on<F: std::future::Future>(f: F) -> F::Output {
    futures::executor::block_on(f)
}

/// Adapts a [`StateDbRead`] to REVM's [`DatabaseRef`] interface.
#[derive(Clone, Debug)]
pub struct StateDbAdapter<S> {
    state: S,
}

impl<S> StateDbAdapter<S> {
    /// Create a new adapter wrapping the given state.
    #[must_use]
    pub const fn new(state: S) -> Self {
        Self { state }
    }

    /// Get the underlying state reference.
    #[must_use]
    pub const fn state(&self) -> &S {
        &self.state
    }
}

impl<S: StateDbRead> DatabaseRef for StateDbAdapter<S> {
    type Error = ExecutionError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        match block_on(self.state.nonce(&address)) {
            Ok(nonce) => {
                let balance = block_on(self.state.balance(&address))?;
                let code_hash = block_on(self.state.code_hash(&address))?;
                Ok(Some(AccountInfo { nonce, balance, code_hash, code: None, account_id: None }))
            }
            Err(StateDbError::AccountNotFound(_)) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        if code_hash == KECCAK256_EMPTY || code_hash == B256::ZERO {
            return Ok(Bytecode::default());
        }
        let bytes = block_on(self.state.code(&code_hash))?;
        Ok(Bytecode::new_raw(bytes))
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        match block_on(self.state.storage(&address, &index)) {
            Ok(value) => Ok(value),
            Err(StateDbError::AccountNotFound(_)) => Ok(U256::ZERO),
            Err(e) => Err(e.into()),
        }
    }

    fn block_hash_ref(&self, _number: u64) -> Result<B256, Self::Error> {
        // Block hash lookups not supported yet
        Ok(B256::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_new() {
        let adapter = StateDbAdapter::new(());
        assert_eq!(adapter.state(), &());
    }
}
