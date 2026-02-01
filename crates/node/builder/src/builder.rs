//! Contains the node builder.

use crate::ConsensusProvider;

/// Node builder.
///
/// A builder for constructing Kora nodes with pluggable consensus providers.
///
/// # Type States
///
/// The builder uses a type-state pattern to ensure components are configured
/// in the correct order:
///
/// - `NodeBuilder<()>`: Initial state, requires a consensus provider
/// - `NodeBuilder<P>`: Consensus provider configured, ready to build
///
/// # Example
///
/// ```rust,ignore
/// use kora_builder::{NodeBuilder, ConsensusProvider};
///
/// let builder = NodeBuilder::new()
///     .with_consensus(my_consensus_provider);
///
/// // Access the configured consensus provider
/// let config = builder.consensus_provider().simplex_config();
/// ```
#[derive(Debug)]
pub struct NodeBuilder<P = ()> {
    /// The consensus provider for the node.
    consensus: P,
}

impl Default for NodeBuilder<()> {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeBuilder<()> {
    /// Creates a new node builder in the initial state.
    #[must_use]
    pub const fn new() -> Self {
        Self { consensus: () }
    }

    /// Configures the consensus provider for the node.
    ///
    /// This transitions the builder from the initial state to a configured state
    /// with the specified consensus provider.
    ///
    /// # Type Parameters
    ///
    /// - `P`: The consensus provider type, must implement [`ConsensusProvider`]
    pub fn with_consensus<P>(self, consensus: P) -> NodeBuilder<P>
    where
        P: ConsensusProvider,
    {
        NodeBuilder { consensus }
    }
}

impl<P> NodeBuilder<P>
where
    P: ConsensusProvider,
{
    /// Returns a reference to the configured consensus provider.
    #[must_use]
    pub const fn consensus_provider(&self) -> &P {
        &self.consensus
    }

    /// Consumes the builder and returns the consensus provider.
    #[must_use]
    pub fn into_consensus_provider(self) -> P {
        self.consensus
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_unit_builder() {
        let builder = NodeBuilder::new();
        assert!(format!("{builder:?}").contains("NodeBuilder"));
    }

    #[test]
    fn test_default_is_same_as_new() {
        let default_builder = NodeBuilder::default();
        let new_builder = NodeBuilder::new();
        assert_eq!(format!("{default_builder:?}"), format!("{new_builder:?}"));
    }
}
