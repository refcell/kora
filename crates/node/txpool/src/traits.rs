//! Mempool trait for transaction pool compatibility.

use std::collections::BTreeSet;

use kora_domain::{Tx, TxId};

/// Mempool provides access to pending transactions for block building.
///
/// Implementations may use different ordering strategies (FIFO, priority, etc).
pub trait Mempool: Clone + Send + Sync + 'static {
    /// Insert a transaction into the mempool.
    ///
    /// Returns `true` if the transaction was newly inserted.
    fn insert(&self, tx: Tx) -> bool;

    /// Build a batch of transactions for inclusion in a block.
    ///
    /// `excluded` contains transaction IDs already included in pending ancestor blocks.
    /// `max_txs` limits the number of transactions returned.
    fn build(&self, max_txs: usize, excluded: &BTreeSet<TxId>) -> Vec<Tx>;

    /// Remove finalized transactions from the mempool.
    fn prune(&self, tx_ids: &[TxId]);

    /// Get the current number of pending transactions.
    fn len(&self) -> usize;

    /// Check if the mempool is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
