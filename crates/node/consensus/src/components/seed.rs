//! In-memory seed tracker implementation.

use std::{collections::BTreeMap, sync::Arc};

use parking_lot::RwLock;

use alloy_primitives::B256;

use crate::traits::{Digest, SeedTracker};

/// Simple in-memory seed tracker.
#[derive(Debug, Clone)]
pub struct InMemorySeedTracker {
    inner: Arc<RwLock<BTreeMap<Digest, B256>>>,
}

impl InMemorySeedTracker {
    /// Create a new seed tracker with genesis seed.
    pub fn new(genesis_digest: Digest) -> Self {
        let mut seeds = BTreeMap::new();
        seeds.insert(genesis_digest, B256::ZERO);
        Self { inner: Arc::new(RwLock::new(seeds)) }
    }

    /// Create an empty seed tracker.
    pub fn empty() -> Self {
        Self { inner: Arc::new(RwLock::new(BTreeMap::new())) }
    }
}

impl Default for InMemorySeedTracker {
    fn default() -> Self {
        Self::empty()
    }
}

impl SeedTracker for InMemorySeedTracker {
    fn get(&self, digest: &Digest) -> Option<B256> {
        self.inner.read().get(digest).copied()
    }

    fn insert(&self, digest: Digest, seed: B256) {
        self.inner.write().insert(digest, seed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_tracker_insert_and_get() {
        let tracker = InMemorySeedTracker::empty();

        let digest = Digest::from([0x01u8; 32]);
        let seed = B256::repeat_byte(0x02);

        assert!(tracker.get(&digest).is_none());

        tracker.insert(digest, seed);
        assert_eq!(tracker.get(&digest), Some(seed));
    }

    #[test]
    fn seed_tracker_genesis() {
        let genesis = Digest::from([0xABu8; 32]);
        let tracker = InMemorySeedTracker::new(genesis);

        assert_eq!(tracker.get(&genesis), Some(B256::ZERO));
    }
}
