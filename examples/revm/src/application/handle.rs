//! Handle for interacting with the application state.
//!
//! The simulation harness uses this handle to:
//! - submit transactions into the node-local mempool, and
//! - query state at a finalized digest for assertions.

use alloy_evm::revm::primitives::{Address, B256, U256};
use kora_domain::{ConsensusDigest, StateRoot, Tx};
use kora_ledger::LedgerService;

#[derive(Clone)]
/// Handle that exposes application queries and submissions to the simulation harness.
pub(crate) struct NodeHandle {
    /// Ledger service used by the simulation harness.
    state: LedgerService,
}

impl NodeHandle {
    pub(crate) const fn new(state: LedgerService) -> Self {
        Self { state }
    }

    pub(crate) async fn submit_tx(&self, tx: Tx) -> bool {
        self.state.submit_tx(tx).await
    }

    pub(crate) async fn query_balance(
        &self,
        digest: ConsensusDigest,
        address: Address,
    ) -> Option<U256> {
        self.state.query_balance(digest, address).await
    }

    pub(crate) async fn query_state_root(&self, digest: ConsensusDigest) -> Option<StateRoot> {
        self.state.query_state_root(digest).await
    }

    pub(crate) async fn query_seed(&self, digest: ConsensusDigest) -> Option<B256> {
        self.state.query_seed(digest).await
    }
}

impl std::fmt::Debug for NodeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeHandle").finish_non_exhaustive()
    }
}
