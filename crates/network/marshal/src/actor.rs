//! Contains the [`ActorInitializer`] which provides defaults for marshal actor configuration.
//!
//! This module provides sensible defaults for configuring the marshal actor
//! and an `init` method for convenient initialization.

use std::num::{NonZeroU64, NonZeroUsize};

use commonware_consensus::{
    Block,
    marshal::{
        Config,
        core::{Actor, Mailbox},
        standard::Standard,
        store::{Blocks, Certificates},
    },
    simplex::scheme::Scheme,
    types::{Epoch, FixedEpocher, Height, ViewDelta},
};
use commonware_cryptography::certificate::Provider;
use commonware_parallel::Sequential;
use commonware_runtime::{BufferPooler, Clock, Metrics, Spawner, Storage, buffer::paged::CacheRef};
use commonware_utils::{Acknowledgement, NZU64, NZUsize};
use rand_core::CryptoRngCore;

/// Provides sensible defaults for marshal actor configuration and initialization.
///
/// # Example
///
/// ```ignore
/// use kora_marshal::ActorInitializer;
///
/// let (actor, mailbox, processed_height) = ActorInitializer::init(
///     context,
///     finalizations_by_height,
///     finalized_blocks,
///     provider,
///     buffer_pool,
///     block_codec_config,
/// ).await;
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ActorInitializer;

impl ActorInitializer {
    /// The default mailbox size.
    pub const DEFAULT_MAILBOX_SIZE: usize = 1024;

    /// The default view retention timeout (10 views).
    pub const DEFAULT_VIEW_RETENTION_TIMEOUT: ViewDelta = ViewDelta::new(10);

    /// The default maximum number of blocks to repair at once.
    pub const DEFAULT_MAX_REPAIR: NonZeroUsize = NZUsize!(10);

    /// The default prunable items per section.
    pub const DEFAULT_PRUNABLE_ITEMS_PER_SECTION: NonZeroU64 = NZU64!(10);

    /// The default replay buffer size.
    pub const DEFAULT_REPLAY_BUFFER: NonZeroUsize = NZUsize!(1024);

    /// The default key write buffer size.
    pub const DEFAULT_KEY_WRITE_BUFFER: NonZeroUsize = NZUsize!(1024);

    /// The default value write buffer size.
    pub const DEFAULT_VALUE_WRITE_BUFFER: NonZeroUsize = NZUsize!(1024);

    /// The default blocks per epoch.
    pub const DEFAULT_BLOCKS_PER_EPOCH: NonZeroU64 = NZU64!(20);

    /// The default partition prefix.
    pub const DEFAULT_PARTITION_PREFIX: &'static str = "marshal";
}

impl ActorInitializer {
    /// Initializes the marshal actor with sensible defaults.
    ///
    /// This method constructs a [`Config`] using the default constants and delegates
    /// to [`Actor::init`] for initialization. Uses [`FixedEpocher`] with
    /// [`DEFAULT_BLOCKS_PER_EPOCH`](Self::DEFAULT_BLOCKS_PER_EPOCH) and
    /// [`Sequential`] strategy.
    ///
    /// # Type Parameters
    ///
    /// - `E`: Runtime context (must implement `CryptoRngCore + Spawner + Metrics + Clock + Storage`)
    /// - `B`: Block type (must implement `Block`)
    /// - `P`: Certificate provider (must implement `Provider<Scope = Epoch>`)
    /// - `FC`: Finalizations certificate storage (must implement `Certificates`)
    /// - `FB`: Finalized blocks storage (must implement `Blocks`)
    /// - `A`: Acknowledgement type (must implement `Acknowledgement`)
    ///
    /// # Returns
    ///
    /// A tuple of `(Actor, Mailbox, Height)` where:
    /// - `Actor` is the initialized marshal actor
    /// - `Mailbox` is the message mailbox for sending consensus messages
    /// - `Height` is the last processed height from storage (or zero if none)
    #[allow(clippy::type_complexity)]
    pub async fn init<E, B, P, FC, FB, A>(
        context: E,
        finalizations_by_height: FC,
        finalized_blocks: FB,
        provider: P,
        page_cache: CacheRef,
        block_codec_config: B::Cfg,
    ) -> (
        Actor<E, Standard<B>, P, FC, FB, FixedEpocher, Sequential, A>,
        Mailbox<P::Scheme, Standard<B>>,
        Height,
    )
    where
        E: BufferPooler + CryptoRngCore + Spawner + Metrics + Clock + Storage,
        B: Block,
        P: Provider<Scope = Epoch, Scheme: Scheme<B::Digest>>,
        FC: Certificates<BlockDigest = B::Digest, Commitment = B::Digest, Scheme = P::Scheme>,
        FB: Blocks<Block = B>,
        A: Acknowledgement,
    {
        let config = Config {
            provider,
            epocher: FixedEpocher::new(Self::DEFAULT_BLOCKS_PER_EPOCH),
            partition_prefix: Self::DEFAULT_PARTITION_PREFIX.to_string(),
            mailbox_size: Self::DEFAULT_MAILBOX_SIZE,
            view_retention_timeout: Self::DEFAULT_VIEW_RETENTION_TIMEOUT,
            prunable_items_per_section: Self::DEFAULT_PRUNABLE_ITEMS_PER_SECTION,
            page_cache,
            replay_buffer: Self::DEFAULT_REPLAY_BUFFER,
            key_write_buffer: Self::DEFAULT_KEY_WRITE_BUFFER,
            value_write_buffer: Self::DEFAULT_VALUE_WRITE_BUFFER,
            block_codec_config,
            max_repair: Self::DEFAULT_MAX_REPAIR,
            max_pending_acks: NZUsize!(1024),
            strategy: Sequential,
        };

        Actor::init(context, finalizations_by_height, finalized_blocks, config).await
    }

    /// Initializes the marshal actor with a custom partition prefix.
    ///
    /// This is the same as [`init`](Self::init) but allows specifying a custom partition prefix
    /// for storage isolation. Useful for testing multiple nodes in the same process.
    #[allow(clippy::type_complexity)]
    pub async fn init_with_partition<E, B, P, FC, FB, A>(
        context: E,
        finalizations_by_height: FC,
        finalized_blocks: FB,
        provider: P,
        page_cache: CacheRef,
        block_codec_config: B::Cfg,
        partition_prefix: impl Into<String>,
    ) -> (
        Actor<E, Standard<B>, P, FC, FB, FixedEpocher, Sequential, A>,
        Mailbox<P::Scheme, Standard<B>>,
        Height,
    )
    where
        E: BufferPooler + CryptoRngCore + Spawner + Metrics + Clock + Storage,
        B: Block,
        P: Provider<Scope = Epoch, Scheme: Scheme<B::Digest>>,
        FC: Certificates<BlockDigest = B::Digest, Commitment = B::Digest, Scheme = P::Scheme>,
        FB: Blocks<Block = B>,
        A: Acknowledgement,
    {
        let config = Config {
            provider,
            epocher: FixedEpocher::new(Self::DEFAULT_BLOCKS_PER_EPOCH),
            partition_prefix: partition_prefix.into(),
            mailbox_size: Self::DEFAULT_MAILBOX_SIZE,
            view_retention_timeout: Self::DEFAULT_VIEW_RETENTION_TIMEOUT,
            prunable_items_per_section: Self::DEFAULT_PRUNABLE_ITEMS_PER_SECTION,
            page_cache,
            replay_buffer: Self::DEFAULT_REPLAY_BUFFER,
            key_write_buffer: Self::DEFAULT_KEY_WRITE_BUFFER,
            value_write_buffer: Self::DEFAULT_VALUE_WRITE_BUFFER,
            block_codec_config,
            max_repair: Self::DEFAULT_MAX_REPAIR,
            max_pending_acks: NZUsize!(1024),
            strategy: Sequential,
        };

        Actor::init(context, finalizations_by_height, finalized_blocks, config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        assert_eq!(ActorInitializer::DEFAULT_MAILBOX_SIZE, 1024);
        assert_eq!(ActorInitializer::DEFAULT_VIEW_RETENTION_TIMEOUT, ViewDelta::new(10));
        assert_eq!(ActorInitializer::DEFAULT_MAX_REPAIR.get(), 10);
        assert_eq!(ActorInitializer::DEFAULT_PRUNABLE_ITEMS_PER_SECTION.get(), 10);
        assert_eq!(ActorInitializer::DEFAULT_REPLAY_BUFFER.get(), 1024);
        assert_eq!(ActorInitializer::DEFAULT_KEY_WRITE_BUFFER.get(), 1024);
        assert_eq!(ActorInitializer::DEFAULT_VALUE_WRITE_BUFFER.get(), 1024);
        assert_eq!(ActorInitializer::DEFAULT_BLOCKS_PER_EPOCH.get(), 20);
        assert_eq!(ActorInitializer::DEFAULT_PARTITION_PREFIX, "marshal");
    }
}
