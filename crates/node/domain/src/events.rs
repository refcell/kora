//! Domain events for the REVM example.

use std::sync::Arc;

use alloy_evm::revm::primitives::B256;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use parking_lot::Mutex;

use super::TxId;
use crate::ConsensusDigest;

/// Ledger-related domain events emitted by the example chain.
#[derive(Clone, Debug)]
pub enum LedgerEvent {
    /// A transaction has been submitted to the ledger.
    TransactionSubmitted(TxId),
    /// A snapshot has been persisted to durable storage.
    SnapshotPersisted(ConsensusDigest),
    /// The randomness seed has been updated for future blocks.
    SeedUpdated(ConsensusDigest, B256),
}

/// Pub-sub registry for ledger events.
#[derive(Clone, Debug)]
pub struct LedgerEvents {
    listeners: Arc<Mutex<Vec<UnboundedSender<LedgerEvent>>>>,
}

impl LedgerEvents {
    /// Create a new, empty event registry.
    #[must_use]
    pub fn new() -> Self {
        Self { listeners: Arc::new(Mutex::new(Vec::new())) }
    }

    /// Publish an event to all current subscribers, dropping closed channels.
    pub fn publish(&self, event: LedgerEvent) {
        let mut guard = self.listeners.lock();
        guard.retain(|sender| sender.unbounded_send(event.clone()).is_ok());
    }

    /// Subscribe to ledger events and receive a stream of updates.
    pub fn subscribe(&self) -> UnboundedReceiver<LedgerEvent> {
        let (sender, receiver) = unbounded();
        self.listeners.lock().push(sender);
        receiver
    }
}

impl Default for LedgerEvents {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::B256;
    use commonware_cryptography::sha256::Digest;

    use super::*;

    #[test]
    fn ledger_events_new() {
        let events = LedgerEvents::new();
        assert_eq!(events.listeners.lock().len(), 0);
    }

    #[test]
    fn ledger_events_default() {
        let events = LedgerEvents::default();
        assert_eq!(events.listeners.lock().len(), 0);
    }

    #[test]
    fn ledger_events_subscribe_adds_listener() {
        let events = LedgerEvents::new();
        let _receiver = events.subscribe();
        assert_eq!(events.listeners.lock().len(), 1);
    }

    #[test]
    fn ledger_events_multiple_subscribers() {
        let events = LedgerEvents::new();
        let _r1 = events.subscribe();
        let _r2 = events.subscribe();
        let _r3 = events.subscribe();
        assert_eq!(events.listeners.lock().len(), 3);
    }

    #[test]
    fn ledger_events_publish_to_subscriber() {
        let events = LedgerEvents::new();
        let mut receiver = events.subscribe();

        let tx_id = TxId(B256::repeat_byte(0x42));
        events.publish(LedgerEvent::TransactionSubmitted(tx_id));

        let received = receiver.try_next().expect("channel open").expect("should receive event");
        if let LedgerEvent::TransactionSubmitted(id) = received {
            assert_eq!(id.0, B256::repeat_byte(0x42));
        } else {
            panic!("wrong event type");
        }
    }

    #[test]
    fn ledger_events_publish_to_multiple_subscribers() {
        let events = LedgerEvents::new();
        let mut r1 = events.subscribe();
        let mut r2 = events.subscribe();

        let tx_id = TxId(B256::repeat_byte(0x01));
        events.publish(LedgerEvent::TransactionSubmitted(tx_id));

        let e1 = r1.try_next().expect("channel open").expect("r1 should receive");
        let e2 = r2.try_next().expect("channel open").expect("r2 should receive");

        assert!(matches!(e1, LedgerEvent::TransactionSubmitted(_)));
        assert!(matches!(e2, LedgerEvent::TransactionSubmitted(_)));
    }

    #[test]
    fn ledger_events_removes_closed_channels() {
        let events = LedgerEvents::new();
        let receiver = events.subscribe();
        assert_eq!(events.listeners.lock().len(), 1);

        drop(receiver);

        let digest: Digest = [0u8; 32].into();
        events.publish(LedgerEvent::SnapshotPersisted(digest));
        assert_eq!(events.listeners.lock().len(), 0);
    }
}
