//! Transport bundle and provider abstraction.

use std::fmt;

use commonware_cryptography::PublicKey;
use commonware_runtime::{Clock, Handle};

use crate::channels::{MarshalChannels, SimplexChannels};

/// Bundle of registered transport channels ready for node use.
///
/// Contains all channel pairs needed for consensus and block dissemination,
/// along with the network handle to keep the transport alive.
pub struct TransportBundle<P: PublicKey, E: Clock> {
    /// Channels for consensus engine (simplex).
    pub simplex: SimplexChannels<P, E>,

    /// Channels for block dissemination and backfill (marshal).
    pub marshal: MarshalChannels<P, E>,

    /// Network handle to keep the transport alive.
    pub handle: Handle<()>,
}

impl<P: PublicKey, E: Clock> fmt::Debug for TransportBundle<P, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransportBundle")
            .field("simplex", &self.simplex)
            .field("marshal", &self.marshal)
            .finish_non_exhaustive()
    }
}

impl<P: PublicKey, E: Clock> TransportBundle<P, E> {
    /// Create a new transport bundle from its components.
    pub const fn new(
        simplex: SimplexChannels<P, E>,
        marshal: MarshalChannels<P, E>,
        handle: Handle<()>,
    ) -> Self {
        Self { simplex, marshal, handle }
    }
}
