//! Provides a default simplex configuration.

use std::time::Duration;

use commonware_consensus::{
    CertifiableAutomaton, Relay, Reporter,
    simplex::{self, elector::Random, types::Activity},
    types::{Epoch, ViewDelta},
};
use commonware_cryptography::{Digest, certificate::Scheme};
use commonware_p2p::Blocker;
use commonware_parallel::Sequential;
use commonware_utils::NZUsize;

use crate::DefaultPool;

/// Default mailbox size for internal consensus channels.
pub const DEFAULT_MAILBOX_SIZE: usize = 1024;

/// Default replay buffer size (1 MiB).
pub const DEFAULT_REPLAY_BUFFER: usize = 1024 * 1024;

/// Default write buffer size (1 MiB).
pub const DEFAULT_WRITE_BUFFER: usize = 1024 * 1024;

/// Default leader timeout (1 second).
pub const DEFAULT_LEADER_TIMEOUT: Duration = Duration::from_secs(1);

/// Default notarization timeout (2 seconds).
pub const DEFAULT_NOTARIZATION_TIMEOUT: Duration = Duration::from_secs(2);

/// Default nullify retry interval (5 seconds).
pub const DEFAULT_NULLIFY_RETRY: Duration = Duration::from_secs(5);

/// Default fetch timeout (1 second).
pub const DEFAULT_FETCH_TIMEOUT: Duration = Duration::from_secs(1);

/// Default activity timeout (20 views).
pub const DEFAULT_ACTIVITY_TIMEOUT: ViewDelta = ViewDelta::new(20);

/// Default skip timeout (10 views).
pub const DEFAULT_SKIP_TIMEOUT: ViewDelta = ViewDelta::new(10);

/// Default number of concurrent fetch requests.
pub const DEFAULT_FETCH_CONCURRENT: usize = 8;

/// The default simplex configuration constructor.
///
/// Creates a [`simplex::Config`] with sensible defaults using:
/// - [`Random`] leader election
/// - [`Sequential`] execution strategy
/// - Default buffer pool from [`DefaultPool`]
/// - Default timing parameters
#[derive(Debug, Clone, Copy)]
pub struct DefaultConfig;

impl DefaultConfig {
    /// Initializes a default [`simplex::Config`].
    ///
    /// # Parameters
    ///
    /// - `partition`: Unique partition name for the consensus engine's journal
    /// - `scheme`: Signing scheme (e.g., BLS12-381 threshold VRF)
    /// - `blocker`: Network blocker for peer management
    /// - `automaton`: Application interface for block production/verification
    /// - `relay`: Relay for broadcasting payloads
    /// - `reporter`: Activity reporter for observability
    #[allow(clippy::type_complexity)]
    pub fn init<S, B, D, A, R, F>(
        partition: impl Into<String>,
        scheme: S,
        blocker: B,
        automaton: A,
        relay: R,
        reporter: F,
    ) -> simplex::Config<S, Random, B, D, A, R, F, Sequential>
    where
        S: Scheme,
        Random: simplex::elector::Config<S>,
        B: Blocker<PublicKey = S::PublicKey>,
        D: Digest,
        A: CertifiableAutomaton<Context = simplex::types::Context<D, S::PublicKey>, Digest = D>,
        R: Relay<Digest = D>,
        F: Reporter<Activity = Activity<S, D>>,
    {
        simplex::Config {
            scheme,
            elector: Random,
            blocker,
            automaton,
            relay,
            reporter,
            strategy: Sequential,
            partition: partition.into(),
            mailbox_size: DEFAULT_MAILBOX_SIZE,
            epoch: Epoch::zero(),
            replay_buffer: NZUsize!(DEFAULT_REPLAY_BUFFER),
            write_buffer: NZUsize!(DEFAULT_WRITE_BUFFER),
            buffer_pool: DefaultPool::init(),
            leader_timeout: DEFAULT_LEADER_TIMEOUT,
            notarization_timeout: DEFAULT_NOTARIZATION_TIMEOUT,
            nullify_retry: DEFAULT_NULLIFY_RETRY,
            fetch_timeout: DEFAULT_FETCH_TIMEOUT,
            activity_timeout: DEFAULT_ACTIVITY_TIMEOUT,
            skip_timeout: DEFAULT_SKIP_TIMEOUT,
            fetch_concurrent: DEFAULT_FETCH_CONCURRENT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mailbox_size_is_1024() {
        assert_eq!(DEFAULT_MAILBOX_SIZE, 1024);
    }

    #[test]
    fn default_replay_buffer_is_1mib() {
        assert_eq!(DEFAULT_REPLAY_BUFFER, 1024 * 1024);
    }

    #[test]
    fn default_write_buffer_is_1mib() {
        assert_eq!(DEFAULT_WRITE_BUFFER, 1024 * 1024);
    }

    #[test]
    fn default_leader_timeout_is_1_second() {
        assert_eq!(DEFAULT_LEADER_TIMEOUT, Duration::from_secs(1));
    }

    #[test]
    fn default_notarization_timeout_is_2_seconds() {
        assert_eq!(DEFAULT_NOTARIZATION_TIMEOUT, Duration::from_secs(2));
    }

    #[test]
    fn default_nullify_retry_is_5_seconds() {
        assert_eq!(DEFAULT_NULLIFY_RETRY, Duration::from_secs(5));
    }

    #[test]
    fn default_fetch_timeout_is_1_second() {
        assert_eq!(DEFAULT_FETCH_TIMEOUT, Duration::from_secs(1));
    }

    #[test]
    fn default_activity_timeout_is_20_views() {
        assert_eq!(DEFAULT_ACTIVITY_TIMEOUT, ViewDelta::new(20));
    }

    #[test]
    fn default_skip_timeout_is_10_views() {
        assert_eq!(DEFAULT_SKIP_TIMEOUT, ViewDelta::new(10));
    }

    #[test]
    fn default_fetch_concurrent_is_8() {
        assert_eq!(DEFAULT_FETCH_CONCURRENT, 8);
    }

    #[test]
    fn default_config_has_debug_impl() {
        let config = DefaultConfig;
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("DefaultConfig"));
    }

    #[test]
    fn default_config_is_copy() {
        let config = DefaultConfig;
        let config2 = config;
        let _ = config;
        let _ = config2;
    }
}
