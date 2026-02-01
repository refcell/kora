//! Provides a default simplex engine constructor.

use commonware_consensus::{
    CertifiableAutomaton, Relay, Reporter,
    simplex::{self, elector::Random, types::Activity},
};
use commonware_cryptography::Digest;
use commonware_p2p::{Blocker, Receiver, Sender};
use commonware_parallel::Sequential;
use commonware_runtime::{Clock, Handle, Metrics, Spawner, Storage};
use rand_core::CryptoRngCore;

use crate::DefaultConfig;

/// The default simplex engine constructor.
///
/// Creates and starts a [`simplex::Engine`] with default configuration.
#[derive(Debug, Clone, Copy)]
pub struct DefaultEngine;

impl DefaultEngine {
    /// Initializes and starts a default simplex engine.
    ///
    /// # Parameters
    ///
    /// - `context`: Runtime context (must implement Clock, CryptoRngCore, Spawner, Storage, Metrics)
    /// - `partition`: Unique partition name for the consensus engine's journal
    /// - `scheme`: Signing scheme (e.g., BLS12-381 threshold VRF)
    /// - `blocker`: Network blocker for peer management
    /// - `automaton`: Application interface for block production/verification
    /// - `relay`: Relay for broadcasting payloads
    /// - `reporter`: Activity reporter for observability
    /// - `vote_network`: Network channel for votes (sender, receiver)
    /// - `certificate_network`: Network channel for certificates (sender, receiver)
    /// - `resolver_network`: Network channel for resolver requests (sender, receiver)
    #[allow(clippy::too_many_arguments)]
    pub fn init<E, S, B, D, A, R, F, VS, VR, CS, CR, RS, RR>(
        context: E,
        partition: impl Into<String>,
        scheme: S,
        blocker: B,
        automaton: A,
        relay: R,
        reporter: F,
        vote_network: (VS, VR),
        certificate_network: (CS, CR),
        resolver_network: (RS, RR),
    ) -> Handle<()>
    where
        E: Clock + CryptoRngCore + Spawner + Storage + Metrics,
        S: simplex::scheme::Scheme<D>,
        Random: simplex::elector::Config<S>,
        B: Blocker<PublicKey = S::PublicKey>,
        D: Digest,
        A: CertifiableAutomaton<Context = simplex::types::Context<D, S::PublicKey>, Digest = D>,
        R: Relay<Digest = D>,
        F: Reporter<Activity = Activity<S, D>>,
        VS: Sender<PublicKey = S::PublicKey>,
        VR: Receiver<PublicKey = S::PublicKey>,
        CS: Sender<PublicKey = S::PublicKey>,
        CR: Receiver<PublicKey = S::PublicKey>,
        RS: Sender<PublicKey = S::PublicKey>,
        RR: Receiver<PublicKey = S::PublicKey>,
    {
        let config: simplex::Config<S, Random, B, D, A, R, F, Sequential> =
            DefaultConfig::init(partition, scheme, blocker, automaton, relay, reporter);
        let engine = simplex::Engine::new(context, config);
        engine.start(vote_network, certificate_network, resolver_network)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_engine_has_debug_impl() {
        let engine = DefaultEngine;
        let debug_str = format!("{:?}", engine);
        assert!(debug_str.contains("DefaultEngine"));
    }

    #[test]
    fn default_engine_is_copy() {
        let engine = DefaultEngine;
        let engine2 = engine;
        let _ = engine;
        let _ = engine2;
    }
}
