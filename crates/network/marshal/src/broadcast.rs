//! Contains the [`BroadcastInitializer`] which initializes the buffered broadcast engine.

use commonware_broadcast::buffered::{Config, Engine, Mailbox};
use commonware_codec::Codec;
use commonware_cryptography::{Committable, Digestible, PublicKey};
use commonware_runtime::{Clock, Metrics, Spawner};

/// Initializes the buffered broadcast engine with sensible defaults.
#[derive(Debug, Clone, Copy)]
pub struct BroadcastInitializer;

impl BroadcastInitializer {
    /// The default mailbox size.
    pub const DEFAULT_MAILBOX_SIZE: usize = 1024;

    /// The default deque size for message buffering.
    pub const DEFAULT_DEQUE_SIZE: usize = 256;

    /// Whether messages are sent with priority by default.
    pub const DEFAULT_PRIORITY: bool = false;
}

impl BroadcastInitializer {
    /// Initializes the buffered broadcast engine.
    ///
    /// Returns the engine and a mailbox for sending messages.
    pub fn init<E, P, M>(
        ctx: E,
        public_key: P,
        codec_config: M::Cfg,
    ) -> (Engine<E, P, M>, Mailbox<P, M>)
    where
        E: Clock + Spawner + Metrics,
        P: PublicKey,
        M: Committable + Digestible + Codec,
    {
        let config = Config {
            public_key,
            mailbox_size: Self::DEFAULT_MAILBOX_SIZE,
            deque_size: Self::DEFAULT_DEQUE_SIZE,
            priority: Self::DEFAULT_PRIORITY,
            codec_config,
        };
        Engine::new(ctx, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        assert_eq!(BroadcastInitializer::DEFAULT_MAILBOX_SIZE, 1024);
        assert_eq!(BroadcastInitializer::DEFAULT_DEQUE_SIZE, 256);
        assert!(!BroadcastInitializer::DEFAULT_PRIORITY);
    }
}
