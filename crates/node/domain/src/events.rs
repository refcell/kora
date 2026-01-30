//! Domain events for the REVM example.

use std::sync::Arc;

use parking_lot::Mutex;

use alloy_evm::revm::primitives::B256;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

use super::TxId;
use crate::ConsensusDigest;

/// Ledger-related domain events emitted by the example chain.
#[derive(Clone, Debug)]
pub enum LedgerEvent {
    #[allow(dead_code)]
    /// A transaction has been submitted to the ledger.
    TransactionSubmitted(TxId),
    #[allow(dead_code)]
    /// A snapshot has been persisted to durable storage.
    SnapshotPersisted(ConsensusDigest),
    #[allow(dead_code)]
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
