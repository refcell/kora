//! Transaction pool implementation.

use std::collections::{BTreeSet, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

use alloy_consensus::{Transaction, TxEnvelope};
use alloy_eips::eip2718::Decodable2718;
use alloy_primitives::{Address, Bytes, B256};
use kora_domain::{Tx, TxId};
use parking_lot::RwLock;
use tracing::{debug, trace, warn};

use crate::{
    config::PoolConfig,
    error::TxPoolError,
    ordering::{OrderedTransaction, SenderQueue},
    traits::Mempool,
    validator::recover_sender_from_envelope,
};

#[derive(Debug)]
struct PoolInner {
    by_hash: HashMap<B256, OrderedTransaction>,
    by_sender: HashMap<Address, SenderQueue>,
    pending_count: usize,
    queued_count: usize,
}

impl PoolInner {
    fn new() -> Self {
        Self {
            by_hash: HashMap::new(),
            by_sender: HashMap::new(),
            pending_count: 0,
            queued_count: 0,
        }
    }

    fn update_counts(&mut self) {
        self.pending_count = self.by_sender.values().map(|q| q.pending_count()).sum();
        self.queued_count = self.by_sender.values().map(|q| q.queued_count()).sum();
    }
}

/// A thread-safe transaction pool with nonce ordering and fee prioritization.
#[derive(Debug)]
pub struct TransactionPool {
    inner: RwLock<PoolInner>,
    config: PoolConfig,
}

impl TransactionPool {
    /// Creates a new transaction pool with the given configuration.
    pub fn new(config: PoolConfig) -> Self {
        Self { inner: RwLock::new(PoolInner::new()), config }
    }

    /// Adds a validated transaction to the pool.
    pub fn add(&self, tx: OrderedTransaction) -> Result<(), TxPoolError> {
        let mut inner = self.inner.write();

        if inner.by_hash.contains_key(&tx.hash) {
            return Err(TxPoolError::AlreadyExists);
        }

        let sender = tx.sender;
        let queue =
            inner.by_sender.entry(sender).or_insert_with(|| SenderQueue::new(sender, tx.nonce));

        if queue.total_count() >= self.config.max_txs_per_sender {
            return Err(TxPoolError::SenderFull(sender));
        }

        if let Some(replaced) = queue.insert(tx.clone()) {
            if replaced.hash == tx.hash {
                return Err(TxPoolError::AlreadyExists);
            }
            inner.by_hash.remove(&replaced.hash);
            debug!(hash = ?replaced.hash, "replaced transaction");
        }

        inner.by_hash.insert(tx.hash, tx);
        inner.update_counts();

        if inner.pending_count > self.config.max_pending_txs {
            warn!(
                count = inner.pending_count,
                max = self.config.max_pending_txs,
                "pool exceeds pending limit"
            );
        }

        if inner.queued_count > self.config.max_queued_txs {
            warn!(
                count = inner.queued_count,
                max = self.config.max_queued_txs,
                "pool exceeds queued limit"
            );
        }

        Ok(())
    }

    /// Returns pending transactions sorted by effective gas price.
    pub fn pending(&self, max_txs: usize) -> Vec<OrderedTransaction> {
        let inner = self.inner.read();

        let mut all_pending: Vec<_> =
            inner.by_sender.values().flat_map(|q| q.pending.iter().cloned()).collect();

        all_pending.sort();
        all_pending.truncate(max_txs);
        all_pending
    }

    /// Returns pending transactions for a specific sender.
    pub fn pending_for_sender(&self, sender: &Address) -> Vec<OrderedTransaction> {
        let inner = self.inner.read();
        inner.by_sender.get(sender).map(|q| q.pending.clone()).unwrap_or_default()
    }

    /// Gets a transaction by its hash.
    pub fn get(&self, hash: &B256) -> Option<OrderedTransaction> {
        self.inner.read().by_hash.get(hash).cloned()
    }

    /// Removes a transaction by its hash.
    pub fn remove(&self, hash: &B256) -> Option<OrderedTransaction> {
        let mut inner = self.inner.write();

        let tx = inner.by_hash.remove(hash)?;
        let sender = tx.sender;

        if let Some(queue) = inner.by_sender.get_mut(&sender) {
            queue.pending.retain(|t| t.hash != *hash);
            queue.queued.retain(|t| t.hash != *hash);

            if queue.is_empty() {
                inner.by_sender.remove(&sender);
            }
        }

        inner.update_counts();
        Some(tx)
    }

    /// Removes confirmed transactions for a sender up to the given nonce.
    pub fn remove_confirmed(&self, sender: &Address, confirmed_nonce: u64) {
        let mut inner = self.inner.write();

        let hashes_to_remove: Vec<B256> = inner
            .by_sender
            .get(sender)
            .map(|queue| {
                queue
                    .pending
                    .iter()
                    .filter(|tx| tx.nonce <= confirmed_nonce)
                    .map(|tx| tx.hash)
                    .collect()
            })
            .unwrap_or_default();

        for hash in hashes_to_remove {
            inner.by_hash.remove(&hash);
        }

        if let Some(queue) = inner.by_sender.get_mut(sender) {
            queue.remove_confirmed(confirmed_nonce);
            if queue.is_empty() {
                inner.by_sender.remove(sender);
            }
        }

        inner.update_counts();
    }

    /// Returns the count of pending (executable) transactions.
    pub fn pending_count(&self) -> usize {
        self.inner.read().pending_count
    }

    /// Returns the count of queued (future nonce) transactions.
    pub fn queued_count(&self) -> usize {
        self.inner.read().queued_count
    }

    /// Returns the total number of transactions in the pool.
    pub fn len(&self) -> usize {
        self.inner.read().by_hash.len()
    }

    /// Returns true if the pool contains no transactions.
    pub fn is_empty(&self) -> bool {
        self.inner.read().by_hash.is_empty()
    }

    /// Returns all senders with transactions in the pool.
    pub fn senders(&self) -> Vec<Address> {
        self.inner.read().by_sender.keys().cloned().collect()
    }

    /// Checks if a transaction with the given hash exists in the pool.
    pub fn contains(&self, hash: &B256) -> bool {
        self.inner.read().by_hash.contains_key(hash)
    }

    /// Removes all transactions from the pool.
    pub fn clear(&self) {
        let mut inner = self.inner.write();
        inner.by_hash.clear();
        inner.by_sender.clear();
        inner.pending_count = 0;
        inner.queued_count = 0;
    }
}

impl Clone for TransactionPool {
    fn clone(&self) -> Self {
        let inner = self.inner.read();
        Self {
            inner: RwLock::new(PoolInner {
                by_hash: inner.by_hash.clone(),
                by_sender: inner.by_sender.clone(),
                pending_count: inner.pending_count,
                queued_count: inner.queued_count,
            }),
            config: self.config.clone(),
        }
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn tx_to_ordered(tx: &Tx) -> Option<OrderedTransaction> {
    let envelope = TxEnvelope::decode_2718(&mut tx.bytes.as_ref()).ok()?;
    let sender = recover_sender_from_envelope(&envelope).ok()?;
    let hash = alloy_primitives::keccak256(alloy_rlp::encode(&envelope));
    let nonce = envelope.nonce();
    let effective_gas_price = match &envelope {
        TxEnvelope::Legacy(tx) => tx.tx().gas_price,
        TxEnvelope::Eip2930(tx) => tx.tx().gas_price,
        TxEnvelope::Eip1559(tx) => tx.tx().max_fee_per_gas,
        TxEnvelope::Eip4844(tx) => tx.tx().tx().max_fee_per_gas,
        TxEnvelope::Eip7702(tx) => tx.tx().max_fee_per_gas,
    };

    Some(OrderedTransaction::new(
        hash,
        sender,
        nonce,
        effective_gas_price,
        current_timestamp(),
        envelope,
    ))
}

impl Mempool for TransactionPool {
    fn insert(&self, tx: Tx) -> bool {
        let Some(ordered) = tx_to_ordered(&tx) else {
            trace!("failed to decode transaction for mempool insert");
            return false;
        };

        match self.add(ordered) {
            Ok(()) => true,
            Err(e) => {
                trace!(?e, "failed to insert transaction");
                false
            }
        }
    }

    fn build(&self, max_txs: usize, excluded: &BTreeSet<TxId>) -> Vec<Tx> {
        let inner = self.inner.read();

        let mut candidates: Vec<_> = inner
            .by_sender
            .values()
            .flat_map(|q| q.pending.iter())
            .filter(|tx| !excluded.contains(&TxId(tx.hash)))
            .cloned()
            .collect();

        candidates.sort();

        let mut result = Vec::with_capacity(max_txs.min(candidates.len()));
        let mut included_senders: HashMap<Address, u64> = HashMap::new();

        for tx in candidates {
            if result.len() >= max_txs {
                break;
            }

            let expected_nonce = included_senders
                .get(&tx.sender)
                .copied()
                .or_else(|| inner.by_sender.get(&tx.sender).map(|q| q.next_nonce))
                .unwrap_or(0);

            if tx.nonce == expected_nonce {
                included_senders.insert(tx.sender, tx.nonce + 1);
                result.push(Tx::new(Bytes::from(alloy_rlp::encode(&tx.envelope))));
            }
        }

        result
    }

    fn prune(&self, tx_ids: &[TxId]) {
        let mut inner = self.inner.write();

        let mut senders_to_check: Vec<Address> = Vec::new();

        for id in tx_ids {
            if let Some(tx) = inner.by_hash.remove(&id.0) {
                senders_to_check.push(tx.sender);
                if let Some(queue) = inner.by_sender.get_mut(&tx.sender) {
                    queue.pending.retain(|t| t.hash != id.0);
                    queue.queued.retain(|t| t.hash != id.0);
                }
            }
        }

        for sender in senders_to_check {
            if let Some(queue) = inner.by_sender.get(&sender) {
                if queue.is_empty() {
                    inner.by_sender.remove(&sender);
                }
            }
        }

        inner.update_counts();
    }

    fn len(&self) -> usize {
        self.inner.read().by_hash.len()
    }
}

#[cfg(test)]
mod tests {
    use alloy_consensus::{SignableTransaction as _, TxEip1559};
    use alloy_primitives::{Signature, TxKind, U256};
    use rand::Rng;

    use super::*;

    fn random_b256() -> B256 {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill(&mut bytes);
        B256::from(bytes)
    }

    fn random_address() -> Address {
        let mut bytes = [0u8; 20];
        rand::thread_rng().fill(&mut bytes);
        Address::from(bytes)
    }

    fn make_ordered_tx(sender: Address, nonce: u64, gas_price: u128) -> OrderedTransaction {
        let inner = TxEip1559 {
            chain_id: 1,
            nonce,
            gas_limit: 21000,
            max_fee_per_gas: gas_price,
            max_priority_fee_per_gas: gas_price,
            to: TxKind::Call(Address::ZERO),
            value: U256::ZERO,
            access_list: Default::default(),
            input: Bytes::new(),
        };
        let sig = Signature::from_scalars_and_parity(B256::ZERO, B256::ZERO, false);
        let signed = inner.into_signed(sig);
        let envelope = TxEnvelope::from(signed);
        OrderedTransaction::new(random_b256(), sender, nonce, gas_price, 0, envelope)
    }

    #[test]
    fn pool_add_and_pending() {
        let config = PoolConfig::default();
        let pool = TransactionPool::new(config);

        let sender = random_address();
        let tx0 = make_ordered_tx(sender, 0, 100);
        let tx1 = make_ordered_tx(sender, 1, 100);

        pool.add(tx0.clone()).unwrap();
        pool.add(tx1.clone()).unwrap();

        assert_eq!(pool.pending_count(), 2);
        assert_eq!(pool.len(), 2);

        let pending = pool.pending(10);
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn pool_duplicate_rejected() {
        let config = PoolConfig::default();
        let pool = TransactionPool::new(config);

        let sender = random_address();
        let tx = make_ordered_tx(sender, 0, 100);
        let tx_dup = tx.clone();

        pool.add(tx).unwrap();
        assert!(matches!(pool.add(tx_dup), Err(TxPoolError::AlreadyExists)));
    }

    #[test]
    fn pool_sender_limit() {
        let config = PoolConfig::default().with_max_txs_per_sender(2);
        let pool = TransactionPool::new(config);

        let sender = random_address();
        pool.add(make_ordered_tx(sender, 0, 100)).unwrap();
        pool.add(make_ordered_tx(sender, 1, 100)).unwrap();

        assert!(matches!(
            pool.add(make_ordered_tx(sender, 2, 100)),
            Err(TxPoolError::SenderFull(_))
        ));
    }

    #[test]
    fn pool_remove() {
        let config = PoolConfig::default();
        let pool = TransactionPool::new(config);

        let sender = random_address();
        let tx = make_ordered_tx(sender, 0, 100);
        let hash = tx.hash;

        pool.add(tx).unwrap();
        assert_eq!(pool.len(), 1);

        pool.remove(&hash);
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn pool_clear() {
        let config = PoolConfig::default();
        let pool = TransactionPool::new(config);

        let sender = random_address();
        pool.add(make_ordered_tx(sender, 0, 100)).unwrap();
        pool.add(make_ordered_tx(sender, 1, 100)).unwrap();

        pool.clear();
        assert!(pool.is_empty());
    }
}
