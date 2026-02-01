//! Stub implementations for running simplex in development.
//!
//! These stubs implement the minimal trait requirements to start the
//! simplex consensus engine. Replace with real implementations as
//! components are built out.

use std::future::Future;

use commonware_consensus::{CertifiableAutomaton, Relay, Reporter, types::Epoch};
use commonware_cryptography::sha256;
use commonware_utils::channels::fallible::OneshotExt as _;
use futures::channel::oneshot;

/// Stub digest type (SHA-256).
pub type StubDigest = sha256::Digest;

/// Stub public key type.
pub type StubPublicKey = commonware_cryptography::ed25519::PublicKey;

/// Create a zero digest.
const fn zero_digest() -> StubDigest {
    sha256::Digest([0u8; 32])
}

/// Stub automaton that does nothing.
///
/// Returns empty digests for all operations.
#[derive(Clone, Debug)]
pub struct StubAutomaton;

#[allow(clippy::manual_async_fn)]
impl commonware_consensus::Automaton for StubAutomaton {
    type Context = commonware_consensus::simplex::types::Context<StubDigest, StubPublicKey>;
    type Digest = StubDigest;

    fn genesis(&mut self, _epoch: Epoch) -> impl Future<Output = Self::Digest> + Send {
        async { zero_digest() }
    }

    #[allow(clippy::async_yields_async)]
    fn propose(
        &mut self,
        _context: Self::Context,
    ) -> impl Future<Output = oneshot::Receiver<Self::Digest>> + Send {
        async {
            let (sender, receiver) = oneshot::channel();
            sender.send_lossy(zero_digest());
            receiver
        }
    }

    #[allow(clippy::async_yields_async)]
    fn verify(
        &mut self,
        _context: Self::Context,
        _payload: Self::Digest,
    ) -> impl Future<Output = oneshot::Receiver<bool>> + Send {
        async {
            let (sender, receiver) = oneshot::channel();
            sender.send_lossy(true);
            receiver
        }
    }
}

impl CertifiableAutomaton for StubAutomaton {}

/// Stub relay that does nothing.
#[derive(Clone, Debug)]
pub struct StubRelay;

#[allow(clippy::manual_async_fn)]
impl Relay for StubRelay {
    type Digest = StubDigest;

    fn broadcast(&mut self, _payload: Self::Digest) -> impl Future<Output = ()> + Send {
        async {}
    }
}

/// Stub reporter that does nothing.
#[derive(Clone, Debug)]
pub struct StubReporter<S> {
    _scheme: std::marker::PhantomData<S>,
}

impl<S> Default for StubReporter<S> {
    fn default() -> Self {
        Self { _scheme: std::marker::PhantomData }
    }
}

impl<S> Reporter for StubReporter<S>
where
    S: commonware_cryptography::certificate::Scheme + Clone + Send + 'static,
{
    type Activity = commonware_consensus::simplex::types::Activity<S, StubDigest>;

    fn report(&mut self, activity: Self::Activity) -> impl Future<Output = ()> + Send {
        use commonware_consensus::simplex::types::Activity;
        async move {
            match activity {
                Activity::Notarize(n) => {
                    tracing::trace!(view = ?n.proposal.round.view(), "notarize vote");
                }
                Activity::Notarization(n) => {
                    tracing::debug!(view = ?n.proposal.round.view(), "notarization");
                }
                Activity::Certification(c) => {
                    tracing::debug!(view = ?c.proposal.round.view(), "certification");
                }
                Activity::Nullify(_) => {
                    tracing::trace!("nullify vote");
                }
                Activity::Nullification(n) => {
                    tracing::debug!(round = ?n.round, "nullification");
                }
                Activity::Finalize(f) => {
                    tracing::trace!(view = ?f.proposal.round.view(), "finalize vote");
                }
                Activity::Finalization(f) => {
                    tracing::info!(view = ?f.proposal.round.view(), "finalization");
                }
                Activity::ConflictingNotarize(_) => {
                    tracing::warn!("conflicting notarize detected");
                }
                Activity::ConflictingFinalize(_) => {
                    tracing::warn!("conflicting finalize detected");
                }
                Activity::NullifyFinalize(_) => {
                    tracing::warn!("nullify-finalize conflict detected");
                }
            }
        }
    }
}

/// Stub blocker that does nothing.
#[derive(Clone, Debug)]
pub struct StubBlocker;

#[allow(clippy::manual_async_fn)]
impl commonware_p2p::Blocker for StubBlocker {
    type PublicKey = StubPublicKey;

    fn block(&mut self, _peer: Self::PublicKey) -> impl Future<Output = ()> + Send {
        async {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_consensus::{Automaton, types::Epoch};

    #[tokio::test]
    async fn stub_automaton_genesis_returns_zero_digest() {
        let mut automaton = StubAutomaton;
        let digest = automaton.genesis(Epoch::zero()).await;
        assert_eq!(digest, zero_digest());
    }

    #[tokio::test]
    async fn stub_automaton_propose_returns_zero_digest() {
        let mut automaton = StubAutomaton;
        let context = commonware_consensus::simplex::types::Context::default();
        let receiver = automaton.propose(context).await;
        let digest = receiver.await.expect("receiver should receive");
        assert_eq!(digest, zero_digest());
    }

    #[tokio::test]
    async fn stub_automaton_verify_returns_true() {
        let mut automaton = StubAutomaton;
        let context = commonware_consensus::simplex::types::Context::default();
        let receiver = automaton.verify(context, zero_digest()).await;
        let result = receiver.await.expect("receiver should receive");
        assert!(result);
    }

    #[tokio::test]
    async fn stub_relay_broadcast_completes() {
        let mut relay = StubRelay;
        relay.broadcast(zero_digest()).await;
    }

    #[tokio::test]
    async fn stub_blocker_block_completes() {
        use commonware_cryptography::ed25519;
        use commonware_p2p::Blocker;

        let mut blocker = StubBlocker;
        let (_, public_key) = ed25519::deterministic_key(b"test_peer");
        blocker.block(public_key).await;
    }

    #[test]
    fn stub_reporter_default() {
        let reporter: StubReporter<commonware_cryptography::ed25519::Scheme> =
            StubReporter::default();
        assert!(format!("{:?}", reporter).contains("StubReporter"));
    }

    #[test]
    fn zero_digest_is_all_zeros() {
        let digest = zero_digest();
        assert!(digest.0.iter().all(|&b| b == 0));
    }
}
