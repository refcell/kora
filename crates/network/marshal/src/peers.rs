//! Contains the [`PeerInitializer`] which initializes the p2p resolver.

use std::time::Duration;

use commonware_consensus::{
    Block,
    marshal::resolver::{
        handler::{Message, Request},
        p2p::Config,
    },
};
use commonware_cryptography::{Digestible, PublicKey};
use commonware_p2p::{Blocker, Provider, Receiver, Sender};
use commonware_resolver::p2p;
use commonware_runtime::{BufferPooler, Clock, Metrics, Spawner};
use commonware_utils::channel::mpsc;
use rand::Rng;

/// Receiver for inbound resolver messages.
pub type ResolverReceiver<B> = mpsc::Receiver<Message<<B as Digestible>::Digest>>;

/// Mailbox used to submit resolver requests.
pub type ResolverMailbox<B, P> = p2p::Mailbox<Request<<B as Digestible>::Digest>, P>;

/// Resolver channels returned by peer initialization.
pub type ResolverChannels<B, P> = (ResolverReceiver<B>, ResolverMailbox<B, P>);

/// Initializes the p2p resolver with the given parameters.
#[derive(Debug, Clone)]
pub struct PeerInitializer;

impl PeerInitializer {
    /// The default mailbox size.
    pub const DEFAULT_MAILBOX_SIZE: usize = 1024;

    /// The default initial delay.
    pub const DEFAULT_INITIAL_DELAY: Duration = Duration::from_millis(200);

    /// The default timeout.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(200);

    /// The fetch retry timeout.
    pub const DEFAULT_FETCH_RETRY_TIMEOUT: Duration = Duration::from_millis(100);

    /// Whether there are priority requests.
    pub const PRIORITY_REQUESTS: bool = false;

    /// Whether there are priority responses.
    pub const PRIORITY_RESPONSES: bool = false;
}

impl PeerInitializer {
    /// Initializes the p2p resolver.
    pub fn init<E, C, Bl, B, S, R, P>(
        ctx: &E,
        public_key: P,
        peer_provider: C,
        blocker: Bl,
        backfill: (S, R),
    ) -> ResolverChannels<B, P>
    where
        E: BufferPooler + Rng + Spawner + Clock + Metrics,
        P: PublicKey,
        C: Provider<PublicKey = P>,
        Bl: Blocker<PublicKey = P>,
        B: Block,
        S: Sender<PublicKey = P>,
        R: Receiver<PublicKey = P>,
    {
        let resolver_cfg = Config {
            public_key,
            peer_provider,
            blocker,
            mailbox_size: Self::DEFAULT_MAILBOX_SIZE,
            initial: Self::DEFAULT_INITIAL_DELAY,
            timeout: Self::DEFAULT_TIMEOUT,
            fetch_retry_timeout: Self::DEFAULT_FETCH_RETRY_TIMEOUT,
            priority_requests: Self::PRIORITY_REQUESTS,
            priority_responses: Self::PRIORITY_RESPONSES,
        };
        commonware_consensus::marshal::resolver::p2p::init(ctx, resolver_cfg, backfill)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        assert_eq!(PeerInitializer::DEFAULT_MAILBOX_SIZE, 1024);
        assert_eq!(PeerInitializer::DEFAULT_INITIAL_DELAY, Duration::from_millis(200));
        assert_eq!(PeerInitializer::DEFAULT_TIMEOUT, Duration::from_millis(200));
        assert_eq!(PeerInitializer::DEFAULT_FETCH_RETRY_TIMEOUT, Duration::from_millis(100));
        assert!(!PeerInitializer::PRIORITY_REQUESTS);
        assert!(!PeerInitializer::PRIORITY_RESPONSES);
    }
}
