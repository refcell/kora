//! Network transport bundle.

use std::fmt;

use commonware_cryptography::PublicKey;
use commonware_p2p::authenticated::discovery;
use commonware_runtime::{Clock, Handle};

use crate::channels::{MarshalChannels, SimplexChannels};

/// Complete network transport bundle.
///
/// Contains everything needed to wire up consensus and application layers:
/// - The oracle for peer management and blocking
/// - All 5 channel pairs grouped by consumer
/// - The network handle to keep it alive
///
/// # Channel Groups
///
/// Channels are grouped by their consumer:
/// - [`SimplexChannels`]: For consensus engine (votes, certs, resolver)
/// - [`MarshalChannels`]: For block dissemination (blocks, backfill)
pub struct NetworkTransport<P: PublicKey, E: Clock> {
    /// Oracle for peer management and Byzantine blocking.
    ///
    /// Implements both [`Manager`](commonware_p2p::Manager) and
    /// [`Blocker`](commonware_p2p::Blocker) traits.
    pub oracle: discovery::Oracle<P>,

    /// Network handle to keep the network task alive.
    ///
    /// Drop this and the network shuts down.
    pub handle: Handle<()>,

    /// Channels for consensus engine (simplex).
    pub simplex: SimplexChannels<P, E>,

    /// Channels for block dissemination and backfill (marshal).
    pub marshal: MarshalChannels<P, E>,
}

impl<P: PublicKey, E: Clock> fmt::Debug for NetworkTransport<P, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetworkTransport")
            .field("simplex", &self.simplex)
            .field("marshal", &self.marshal)
            .finish_non_exhaustive()
    }
}
