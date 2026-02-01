//! Node state management for RPC endpoints.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// Shared node state that can be updated by the consensus engine.
#[derive(Debug, Clone)]
pub struct NodeState {
    inner: Arc<NodeStateInner>,
}

#[derive(Debug)]
struct NodeStateInner {
    chain_id: u64,
    validator_index: u32,
    started_at: Instant,
    current_view: AtomicU64,
    finalized_count: AtomicU64,
    proposed_count: AtomicU64,
    nullified_count: AtomicU64,
    peer_count: AtomicU64,
    is_leader: RwLock<bool>,
}

impl NodeState {
    /// Create a new node state.
    #[must_use]
    pub fn new(chain_id: u64, validator_index: u32) -> Self {
        Self {
            inner: Arc::new(NodeStateInner {
                chain_id,
                validator_index,
                started_at: Instant::now(),
                current_view: AtomicU64::new(0),
                finalized_count: AtomicU64::new(0),
                proposed_count: AtomicU64::new(0),
                nullified_count: AtomicU64::new(0),
                peer_count: AtomicU64::new(0),
                is_leader: RwLock::new(false),
            }),
        }
    }

    /// Update the current view.
    pub fn set_view(&self, view: u64) {
        self.inner.current_view.store(view, Ordering::Relaxed);
        // Compute leader: view mod 4 (for 4 validators)
        let is_leader = (view % 4) as u32 == self.inner.validator_index;
        *self.inner.is_leader.write() = is_leader;
    }

    /// Increment finalized block count.
    pub fn inc_finalized(&self) {
        self.inner.finalized_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment proposed block count.
    pub fn inc_proposed(&self) {
        self.inner.proposed_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment nullified round count.
    pub fn inc_nullified(&self) {
        self.inner.nullified_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Update peer count.
    pub fn set_peer_count(&self, count: u64) {
        self.inner.peer_count.store(count, Ordering::Relaxed);
    }

    /// Get current node status.
    pub fn status(&self) -> NodeStatus {
        NodeStatus {
            chain_id: self.inner.chain_id,
            validator_index: self.inner.validator_index,
            uptime_secs: self.inner.started_at.elapsed().as_secs(),
            current_view: self.inner.current_view.load(Ordering::Relaxed),
            finalized_count: self.inner.finalized_count.load(Ordering::Relaxed),
            proposed_count: self.inner.proposed_count.load(Ordering::Relaxed),
            nullified_count: self.inner.nullified_count.load(Ordering::Relaxed),
            peer_count: self.inner.peer_count.load(Ordering::Relaxed),
            is_leader: *self.inner.is_leader.read(),
        }
    }
}

/// Serializable node status for RPC responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStatus {
    /// Chain ID.
    pub chain_id: u64,
    /// This validator's index (0-3).
    pub validator_index: u32,
    /// Seconds since node started.
    pub uptime_secs: u64,
    /// Current consensus view number.
    pub current_view: u64,
    /// Number of finalized blocks.
    pub finalized_count: u64,
    /// Number of blocks proposed by this node.
    pub proposed_count: u64,
    /// Number of nullified rounds.
    pub nullified_count: u64,
    /// Number of connected peers.
    pub peer_count: u64,
    /// Whether this node is the current leader.
    pub is_leader: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_status_serde_roundtrip() {
        let status = NodeStatus {
            chain_id: 1337,
            validator_index: 2,
            uptime_secs: 3600,
            current_view: 100,
            finalized_count: 50,
            proposed_count: 10,
            nullified_count: 5,
            peer_count: 3,
            is_leader: true,
        };

        let json = serde_json::to_string(&status).unwrap();
        let parsed: NodeStatus = serde_json::from_str(&json).unwrap();

        assert_eq!(status.chain_id, parsed.chain_id);
        assert_eq!(status.validator_index, parsed.validator_index);
        assert_eq!(status.uptime_secs, parsed.uptime_secs);
        assert_eq!(status.current_view, parsed.current_view);
        assert_eq!(status.finalized_count, parsed.finalized_count);
        assert_eq!(status.proposed_count, parsed.proposed_count);
        assert_eq!(status.nullified_count, parsed.nullified_count);
        assert_eq!(status.peer_count, parsed.peer_count);
        assert_eq!(status.is_leader, parsed.is_leader);
    }

    #[test]
    fn node_status_json_uses_camel_case() {
        let status = NodeStatus {
            chain_id: 1,
            validator_index: 0,
            uptime_secs: 0,
            current_view: 0,
            finalized_count: 0,
            proposed_count: 0,
            nullified_count: 0,
            peer_count: 0,
            is_leader: false,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("chainId"));
        assert!(json.contains("validatorIndex"));
        assert!(json.contains("uptimeSecs"));
        assert!(json.contains("currentView"));
        assert!(json.contains("finalizedCount"));
        assert!(json.contains("proposedCount"));
        assert!(json.contains("nullifiedCount"));
        assert!(json.contains("peerCount"));
        assert!(json.contains("isLeader"));
    }

    #[test]
    fn node_state_new() {
        let state = NodeState::new(1337, 2);
        let status = state.status();
        assert_eq!(status.chain_id, 1337);
        assert_eq!(status.validator_index, 2);
        assert!(!status.is_leader);
    }

    #[test]
    fn node_state_set_view() {
        let state = NodeState::new(1, 0);
        state.set_view(4);
        let status = state.status();
        assert_eq!(status.current_view, 4);
        assert!(status.is_leader);
    }

    #[test]
    fn node_state_inc_counters() {
        let state = NodeState::new(1, 0);
        state.inc_finalized();
        state.inc_finalized();
        state.inc_proposed();
        state.inc_nullified();

        let status = state.status();
        assert_eq!(status.finalized_count, 2);
        assert_eq!(status.proposed_count, 1);
        assert_eq!(status.nullified_count, 1);
    }

    #[test]
    fn node_state_set_peer_count() {
        let state = NodeState::new(1, 0);
        state.set_peer_count(5);
        assert_eq!(state.status().peer_count, 5);
    }
}
