//! Provides a default buffer pool implementation.

use commonware_runtime::buffer::PoolRef;
use commonware_utils::{NZU16, NZUsize};

/// Default page size in bytes (64 KiB).
///
/// This matches the minimum floor required by consensus message serialization
/// (BLS signatures, finalization certificates, etc.).
pub const DEFAULT_PAGE_SIZE: u16 = 65_535;

/// Default pool capacity (number of pages).
pub const DEFAULT_POOL_CAPACITY: usize = 10_000;

/// The default buffer pool constructor.
#[derive(Debug, Clone, Copy)]
pub struct DefaultPool;

impl DefaultPool {
    /// Initializes a default [`PoolRef`].
    ///
    /// Uses a page size of 16 KiB and a capacity of 10,000 pages.
    pub fn init() -> PoolRef {
        PoolRef::new(NZU16!(DEFAULT_PAGE_SIZE), NZUsize!(DEFAULT_POOL_CAPACITY))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_page_size_is_64kib() {
        assert_eq!(DEFAULT_PAGE_SIZE, 65_535);
    }

    #[test]
    fn default_pool_capacity_is_10000() {
        assert_eq!(DEFAULT_POOL_CAPACITY, 10_000);
    }

    #[test]
    fn default_pool_has_debug_impl() {
        let pool = DefaultPool;
        let debug_str = format!("{:?}", pool);
        assert!(debug_str.contains("DefaultPool"));
    }

    #[test]
    fn default_pool_is_copy() {
        let pool = DefaultPool;
        let pool2 = pool;
        let _ = pool;
        let _ = pool2;
    }

    #[test]
    fn pool_ref_can_be_initialized() {
        let _pool = DefaultPool::init();
    }
}
