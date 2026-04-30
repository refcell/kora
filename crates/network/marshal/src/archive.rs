//! Contains the [`ArchiveInitializer`] which initializes immutable archive storage.

use std::num::{NonZeroU16, NonZeroU64, NonZeroUsize};

use commonware_codec::Codec;
use commonware_runtime::{BufferPooler, Clock, Metrics, Spawner, Storage, buffer::paged::CacheRef};
use commonware_storage::archive::immutable::{Archive, Config};
use commonware_utils::{NZU16, NZU64, NZUsize, sequence::Array};

/// Initializes immutable archive storage with sensible defaults.
#[derive(Debug, Clone, Copy)]
pub struct ArchiveInitializer;

impl ArchiveInitializer {
    /// The default freezer table initial size.
    pub const DEFAULT_FREEZER_TABLE_INITIAL_SIZE: u32 = 65_536;

    /// The default freezer table resize frequency.
    pub const DEFAULT_FREEZER_TABLE_RESIZE_FREQUENCY: u8 = 4;

    /// The default freezer table resize chunk size.
    pub const DEFAULT_FREEZER_TABLE_RESIZE_CHUNK_SIZE: u32 = 16_384;

    /// The default freezer value target size.
    pub const DEFAULT_FREEZER_VALUE_TARGET_SIZE: u64 = 1024;

    /// The default compression level (zstd level 3).
    pub const DEFAULT_COMPRESSION_LEVEL: Option<u8> = Some(3);

    /// The default items per section.
    pub const DEFAULT_ITEMS_PER_SECTION: NonZeroU64 = NZU64!(1024);

    /// The default write buffer size.
    pub const DEFAULT_WRITE_BUFFER: NonZeroUsize = NZUsize!(1024);

    /// The default replay buffer size.
    pub const DEFAULT_REPLAY_BUFFER: NonZeroUsize = NZUsize!(1024);

    /// The default page size.
    pub const DEFAULT_PAGE_SIZE: NonZeroU16 = NZU16!(1024);

    /// The default page cache size.
    pub const DEFAULT_PAGE_CACHE_SIZE: NonZeroUsize = NZUsize!(10);

    /// The default partition prefix for finalizations archive.
    pub const DEFAULT_FINALIZATIONS_PREFIX: &'static str = "finalizations";

    /// The default partition prefix for blocks archive.
    pub const DEFAULT_BLOCKS_PREFIX: &'static str = "blocks";
}

impl ArchiveInitializer {
    /// Initializes an immutable archive with a custom partition prefix.
    ///
    /// The `partition_prefix` is used to namespace all storage partitions.
    /// The `codec_config` configures serialization for stored values.
    ///
    /// Type parameters:
    /// - `E`: Runtime context (must implement `Spawner + Storage + Metrics + Clock`)
    /// - `K`: Key type (must implement `Array`)
    /// - `V`: Value type (must implement `Codec`)
    pub async fn init<E, K, V>(
        ctx: E,
        partition_prefix: impl Into<String>,
        codec_config: V::Cfg,
    ) -> Result<Archive<E, K, V>, commonware_storage::archive::Error>
    where
        E: BufferPooler + Spawner + Storage + Metrics + Clock + Clone,
        K: Array,
        V: Codec + Send + Sync,
    {
        let prefix = partition_prefix.into();
        let config = Config {
            metadata_partition: format!("{prefix}-metadata"),
            freezer_table_partition: format!("{prefix}-freezer-table"),
            freezer_table_initial_size: Self::DEFAULT_FREEZER_TABLE_INITIAL_SIZE,
            freezer_table_resize_frequency: Self::DEFAULT_FREEZER_TABLE_RESIZE_FREQUENCY,
            freezer_table_resize_chunk_size: Self::DEFAULT_FREEZER_TABLE_RESIZE_CHUNK_SIZE,
            freezer_key_partition: format!("{prefix}-freezer-key"),
            freezer_key_page_cache: CacheRef::from_pooler(
                &ctx,
                Self::DEFAULT_PAGE_SIZE,
                Self::DEFAULT_PAGE_CACHE_SIZE,
            ),
            freezer_value_partition: format!("{prefix}-freezer-value"),
            freezer_value_target_size: Self::DEFAULT_FREEZER_VALUE_TARGET_SIZE,
            freezer_value_compression: Self::DEFAULT_COMPRESSION_LEVEL,
            ordinal_partition: format!("{prefix}-ordinal"),
            items_per_section: Self::DEFAULT_ITEMS_PER_SECTION,
            freezer_key_write_buffer: Self::DEFAULT_WRITE_BUFFER,
            freezer_value_write_buffer: Self::DEFAULT_WRITE_BUFFER,
            ordinal_write_buffer: Self::DEFAULT_WRITE_BUFFER,
            replay_buffer: Self::DEFAULT_REPLAY_BUFFER,
            codec_config,
        };
        Archive::init(ctx, config).await
    }

    /// Initializes a finalizations archive with the default prefix.
    ///
    /// Uses [`DEFAULT_FINALIZATIONS_PREFIX`](Self::DEFAULT_FINALIZATIONS_PREFIX) as the partition prefix.
    pub async fn init_finalizations<E, K, V>(
        ctx: E,
        codec_config: V::Cfg,
    ) -> Result<Archive<E, K, V>, commonware_storage::archive::Error>
    where
        E: BufferPooler + Spawner + Storage + Metrics + Clock + Clone,
        K: Array,
        V: Codec + Send + Sync,
    {
        Self::init(ctx, Self::DEFAULT_FINALIZATIONS_PREFIX, codec_config).await
    }

    /// Initializes a blocks archive with the default prefix.
    ///
    /// Uses [`DEFAULT_BLOCKS_PREFIX`](Self::DEFAULT_BLOCKS_PREFIX) as the partition prefix.
    pub async fn init_blocks<E, K, V>(
        ctx: E,
        codec_config: V::Cfg,
    ) -> Result<Archive<E, K, V>, commonware_storage::archive::Error>
    where
        E: BufferPooler + Spawner + Storage + Metrics + Clock + Clone,
        K: Array,
        V: Codec + Send + Sync,
    {
        Self::init(ctx, Self::DEFAULT_BLOCKS_PREFIX, codec_config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        assert_eq!(ArchiveInitializer::DEFAULT_FREEZER_TABLE_INITIAL_SIZE, 65_536);
        assert_eq!(ArchiveInitializer::DEFAULT_FREEZER_TABLE_RESIZE_FREQUENCY, 4);
        assert_eq!(ArchiveInitializer::DEFAULT_FREEZER_TABLE_RESIZE_CHUNK_SIZE, 16_384);
        assert_eq!(ArchiveInitializer::DEFAULT_FREEZER_VALUE_TARGET_SIZE, 1024);
        assert_eq!(ArchiveInitializer::DEFAULT_COMPRESSION_LEVEL, Some(3));
        assert_eq!(ArchiveInitializer::DEFAULT_ITEMS_PER_SECTION.get(), 1024);
        assert_eq!(ArchiveInitializer::DEFAULT_WRITE_BUFFER.get(), 1024);
        assert_eq!(ArchiveInitializer::DEFAULT_REPLAY_BUFFER.get(), 1024);
        assert_eq!(ArchiveInitializer::DEFAULT_PAGE_SIZE.get(), 1024);
        assert_eq!(ArchiveInitializer::DEFAULT_PAGE_CACHE_SIZE.get(), 10);
        assert_eq!(ArchiveInitializer::DEFAULT_FINALIZATIONS_PREFIX, "finalizations");
        assert_eq!(ArchiveInitializer::DEFAULT_BLOCKS_PREFIX, "blocks");
    }
}
