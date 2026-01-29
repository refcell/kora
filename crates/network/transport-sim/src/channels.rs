//! Channel types for simulated transport.

use commonware_cryptography::PublicKey;
use commonware_p2p::simulated;

use crate::SimContext;

/// Type alias for simulated channel sender.
pub type Sender<P> = simulated::Sender<P, SimContext>;

/// Type alias for simulated channel receiver.
pub type Receiver<P> = simulated::Receiver<P>;

/// Simplex consensus channels for simulated transport.
#[allow(missing_debug_implementations)]
pub struct SimSimplexChannels<P: PublicKey> {
    /// Voting traffic channel.
    pub votes: (Sender<P>, Receiver<P>),
    /// Certificate gossip channel.
    pub certs: (Sender<P>, Receiver<P>),
    /// Resolver control channel.
    pub resolver: (Sender<P>, Receiver<P>),
}

/// Marshal channels for simulated transport.
#[allow(missing_debug_implementations)]
pub struct SimMarshalChannels<P: PublicKey> {
    /// Block broadcast channel.
    pub blocks: (Sender<P>, Receiver<P>),
    /// Backfill response channel.
    pub backfill: (Sender<P>, Receiver<P>),
}
