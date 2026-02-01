//! In-memory mempool implementation.

use std::{collections::BTreeMap, sync::Arc};

use kora_domain::Tx;
use parking_lot::RwLock;

use crate::traits::{Mempool, TxId};

/// Simple in-memory mempool backed by a BTreeMap.
#[derive(Debug, Clone)]
pub struct InMemoryMempool {
    inner: Arc<RwLock<BTreeMap<TxId, Tx>>>,
}

impl InMemoryMempool {
    /// Create a new empty mempool.
    #[must_use]
    pub fn new() -> Self {
        Self { inner: Arc::new(RwLock::new(BTreeMap::new())) }
    }
}

impl Default for InMemoryMempool {
    fn default() -> Self {
        Self::new()
    }
}

impl Mempool for InMemoryMempool {
    fn insert(&self, tx: Tx) -> bool {
        let id = tx.id();
        let mut inner = self.inner.write();
        inner.insert(id, tx).is_none()
    }

    fn build(&self, max_txs: usize, excluded: &std::collections::BTreeSet<TxId>) -> Vec<Tx> {
        let inner = self.inner.read();
        inner
            .iter()
            .filter(|(id, _)| !excluded.contains(id))
            .take(max_txs)
            .map(|(_, tx)| tx.clone())
            .collect()
    }

    fn prune(&self, tx_ids: &[TxId]) {
        let mut inner = self.inner.write();
        for id in tx_ids {
            inner.remove(id);
        }
    }

    fn len(&self) -> usize {
        self.inner.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mempool_insert_and_build() {
        let mempool = InMemoryMempool::new();

        let tx1 = Tx::new(vec![1, 2, 3].into());
        let tx2 = Tx::new(vec![4, 5, 6].into());

        assert!(mempool.insert(tx1.clone()));
        assert!(mempool.insert(tx2));
        assert!(!mempool.insert(tx1)); // Duplicate

        assert_eq!(mempool.len(), 2);

        let txs = mempool.build(10, &std::collections::BTreeSet::new());
        assert_eq!(txs.len(), 2);
    }

    #[test]
    fn mempool_prune() {
        let mempool = InMemoryMempool::new();

        let tx = Tx::new(vec![1, 2, 3].into());
        let id = tx.id();

        mempool.insert(tx);
        assert_eq!(mempool.len(), 1);

        mempool.prune(&[id]);
        assert_eq!(mempool.len(), 0);
    }

    #[test]
    fn mempool_build_with_exclusions() {
        let mempool = InMemoryMempool::new();

        let tx1 = Tx::new(vec![1, 2, 3].into());
        let tx2 = Tx::new(vec![4, 5, 6].into());
        let id1 = tx1.id();

        mempool.insert(tx1);
        mempool.insert(tx2.clone());

        let mut excluded = std::collections::BTreeSet::new();
        excluded.insert(id1);

        let txs = mempool.build(10, &excluded);
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0], tx2);
    }
}
