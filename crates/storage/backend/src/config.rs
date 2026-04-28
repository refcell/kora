//! Configuration types for the backend.

use std::num::{NonZeroU16, NonZeroUsize};

use commonware_utils::{NZU16, NZUsize};

const DEFAULT_PAGE_SIZE: NonZeroU16 = NZU16!(16 * 1024);
const DEFAULT_PAGE_CACHE_SIZE: NonZeroUsize = NZUsize!(1_024);

/// Configuration for the full QMDB backend.
#[derive(Clone)]
pub struct QmdbBackendConfig {
    /// Prefix used to derive partition names.
    pub partition_prefix: String,
    /// Logical page size used by the underlying QMDB page cache.
    pub page_size: NonZeroU16,
    /// Number of logical pages retained by the underlying QMDB page cache.
    pub page_cache_size: NonZeroUsize,
}

impl QmdbBackendConfig {
    /// Create a new backend config for the given partition prefix.
    pub fn new(partition_prefix: impl Into<String>) -> Self {
        Self {
            partition_prefix: partition_prefix.into(),
            page_size: DEFAULT_PAGE_SIZE,
            page_cache_size: DEFAULT_PAGE_CACHE_SIZE,
        }
    }

    /// Override the QMDB page cache settings.
    #[must_use]
    pub const fn with_page_cache(
        mut self,
        page_size: NonZeroU16,
        page_cache_size: NonZeroUsize,
    ) -> Self {
        self.page_size = page_size;
        self.page_cache_size = page_cache_size;
        self
    }
}

impl std::fmt::Debug for QmdbBackendConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QmdbBackendConfig")
            .field("partition_prefix", &self.partition_prefix)
            .finish()
    }
}
