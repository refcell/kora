//! Stub implementations for running simplex in development.
//!
//! These stubs implement the minimal trait requirements to start the
//! simplex consensus engine. Replace with real implementations as
//! components are built out.

use std::future::Future;

use commonware_consensus::{CertifiableAutomaton, Relay, Reporter, types::Epoch};
use commonware_cryptography::sha256;
use commonware_utils::channel::{fallible::OneshotExt as _, oneshot};

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
    type PublicKey = StubPublicKey;
    type Plan = ();

    fn broadcast(
        &mut self,
        _payload: Self::Digest,
        _plan: Self::Plan,
    ) -> impl Future<Output = ()> + Send {
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
