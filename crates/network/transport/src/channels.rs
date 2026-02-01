//! Channel groupings for transport consumers.

use std::fmt;

use commonware_cryptography::PublicKey;
use commonware_p2p::authenticated::discovery;
use commonware_runtime::Clock;

/// Channel ID for vote messages.
pub const CHANNEL_VOTES: u64 = 0;

/// Channel ID for certificate messages.
pub const CHANNEL_CERTS: u64 = 1;

/// Channel ID for resolver messages.
pub const CHANNEL_RESOLVER: u64 = 2;

/// Channel ID for block broadcast messages.
pub const CHANNEL_BLOCKS: u64 = 3;

/// Channel ID for backfill messages.
pub const CHANNEL_BACKFILL: u64 = 4;

/// Type alias for channel sender.
pub type Sender<P, E> = discovery::Sender<P, E>;

/// Type alias for channel receiver.
pub type Receiver<P> = discovery::Receiver<P>;

/// Channels for the simplex consensus engine.
///
/// These channels handle voting and certificate gossip for consensus.
pub struct SimplexChannels<P: PublicKey, E: Clock> {
    /// Voting traffic: proposed values from leaders.
    pub votes: (Sender<P, E>, Receiver<P>),

    /// Certificate gossip: notarization/finalization certificates.
    pub certs: (Sender<P, E>, Receiver<P>),

    /// Resolver control: fetches for missing dependencies.
    pub resolver: (Sender<P, E>, Receiver<P>),
}

impl<P: PublicKey, E: Clock> fmt::Debug for SimplexChannels<P, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimplexChannels").finish_non_exhaustive()
    }
}

/// Channels for the marshal block dissemination layer.
///
/// These channels handle block broadcast and backfill requests.
pub struct MarshalChannels<P: PublicKey, E: Clock> {
    /// Full block broadcast: reliable dissemination of complete blocks.
    pub blocks: (Sender<P, E>, Receiver<P>),

    /// Backfill responses: served by resolver protocol.
    pub backfill: (Sender<P, E>, Receiver<P>),
}

impl<P: PublicKey, E: Clock> fmt::Debug for MarshalChannels<P, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MarshalChannels").finish_non_exhaustive()
    }
}
