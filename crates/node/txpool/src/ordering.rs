//! Transaction ordering for block building.

use std::cmp::Ordering;

use alloy_consensus::TxEnvelope;
use alloy_primitives::{Address, B256};

/// A transaction with ordering metadata for pool management.
#[derive(Debug, Clone)]
pub struct OrderedTransaction {
    /// Transaction hash.
    pub hash: B256,
    /// Sender address recovered from signature.
    pub sender: Address,
    /// Transaction nonce.
    pub nonce: u64,
    /// Effective gas price for ordering.
    pub effective_gas_price: u128,
    /// Timestamp when transaction was received.
    pub timestamp: u64,
    /// The decoded transaction envelope.
    pub envelope: TxEnvelope,
}

impl OrderedTransaction {
    /// Creates a new ordered transaction.
    pub const fn new(
        hash: B256,
        sender: Address,
        nonce: u64,
        effective_gas_price: u128,
        timestamp: u64,
        envelope: TxEnvelope,
    ) -> Self {
        Self { hash, sender, nonce, effective_gas_price, timestamp, envelope }
    }

    /// Calculates the effective tip given a base fee.
    pub fn effective_tip(&self, base_fee: Option<u128>) -> u128 {
        base_fee
            .map_or(self.effective_gas_price, |base| self.effective_gas_price.saturating_sub(base))
    }
}

impl PartialEq for OrderedTransaction {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Eq for OrderedTransaction {}

impl PartialOrd for OrderedTransaction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedTransaction {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .effective_gas_price
            .cmp(&self.effective_gas_price)
            .then_with(|| self.timestamp.cmp(&other.timestamp))
            .then_with(|| self.hash.cmp(&other.hash))
    }
}

/// Per-sender queue managing nonce-ordered transactions.
#[derive(Debug, Clone)]
pub struct SenderQueue {
    /// Sender address.
    pub sender: Address,
    /// Next expected nonce for pending transactions.
    pub next_nonce: u64,
    /// Executable transactions with consecutive nonces.
    pub pending: Vec<OrderedTransaction>,
    /// Future transactions waiting for nonce gaps to fill.
    pub queued: Vec<OrderedTransaction>,
}

impl SenderQueue {
    /// Creates a new sender queue.
    #[must_use]
    pub fn new(sender: Address, initial_nonce: u64) -> Self {
        Self { sender, next_nonce: initial_nonce, pending: Vec::new(), queued: Vec::new() }
    }

    /// Inserts a transaction into the queue.
    /// Returns the replaced transaction if one was displaced.
    pub fn insert(&mut self, tx: OrderedTransaction) -> Option<OrderedTransaction> {
        if tx.nonce < self.next_nonce {
            return Some(tx);
        }

        if tx.nonce == self.next_nonce + self.pending.len() as u64 {
            self.pending.push(tx);
            self.promote_queued();
            None
        } else if tx.nonce > self.next_nonce + self.pending.len() as u64 {
            let pos =
                self.queued.binary_search_by(|q| q.nonce.cmp(&tx.nonce)).unwrap_or_else(|p| p);
            self.queued.insert(pos, tx);
            None
        } else {
            let idx = (tx.nonce - self.next_nonce) as usize;
            if idx < self.pending.len() {
                let existing = &self.pending[idx];
                if tx.effective_gas_price > existing.effective_gas_price {
                    let old = std::mem::replace(&mut self.pending[idx], tx);
                    return Some(old);
                }
            }
            Some(tx)
        }
    }

    fn promote_queued(&mut self) {
        while let Some(first) = self.queued.first() {
            if first.nonce == self.next_nonce + self.pending.len() as u64 {
                let tx = self.queued.remove(0);
                self.pending.push(tx);
            } else {
                break;
            }
        }
    }

    /// Removes transactions with nonces up to and including confirmed_nonce.
    pub fn remove_confirmed(&mut self, confirmed_nonce: u64) {
        self.pending.retain(|tx| tx.nonce > confirmed_nonce);
        self.queued.retain(|tx| tx.nonce > confirmed_nonce);
        if confirmed_nonce >= self.next_nonce {
            self.next_nonce = confirmed_nonce + 1;
        }
    }

    /// Returns the count of pending transactions.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Returns the count of queued transactions.
    pub fn queued_count(&self) -> usize {
        self.queued.len()
    }

    /// Returns the total count of transactions.
    pub fn total_count(&self) -> usize {
        self.pending.len() + self.queued.len()
    }

    /// Returns true if the queue has no transactions.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.queued.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use alloy_consensus::{SignableTransaction as _, TxEip1559};
    use alloy_primitives::{Address, Bytes, Signature, TxKind, U256};
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

    fn make_tx(nonce: u64, gas_price: u128) -> OrderedTransaction {
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
        let sig = Signature::from_scalars_and_parity(
            alloy_primitives::B256::ZERO,
            alloy_primitives::B256::ZERO,
            false,
        );
        let signed = inner.into_signed(sig);
        let envelope = TxEnvelope::from(signed);
        OrderedTransaction::new(random_b256(), random_address(), nonce, gas_price, 0, envelope)
    }

    #[test]
    fn sender_queue_insert_sequential() {
        let sender = random_address();
        let mut queue = SenderQueue::new(sender, 0);

        let tx0 = make_tx(0, 100);
        let tx1 = make_tx(1, 100);
        let tx2 = make_tx(2, 100);

        assert!(queue.insert(tx0).is_none());
        assert!(queue.insert(tx1).is_none());
        assert!(queue.insert(tx2).is_none());

        assert_eq!(queue.pending_count(), 3);
        assert_eq!(queue.queued_count(), 0);
    }

    #[test]
    fn sender_queue_insert_with_gap() {
        let sender = random_address();
        let mut queue = SenderQueue::new(sender, 0);

        let tx0 = make_tx(0, 100);
        let tx2 = make_tx(2, 100);
        let tx1 = make_tx(1, 100);

        assert!(queue.insert(tx0).is_none());
        assert!(queue.insert(tx2).is_none());

        assert_eq!(queue.pending_count(), 1);
        assert_eq!(queue.queued_count(), 1);

        assert!(queue.insert(tx1).is_none());

        assert_eq!(queue.pending_count(), 3);
        assert_eq!(queue.queued_count(), 0);
    }

    #[test]
    fn ordered_transaction_ordering() {
        let tx1 = make_tx(0, 100);
        let tx2 = make_tx(0, 200);

        assert!(tx2 < tx1);
    }
}
