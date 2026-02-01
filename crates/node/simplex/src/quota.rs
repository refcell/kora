//! Provides a default quota implementation.

use commonware_runtime::Quota;
use commonware_utils::NZU32;

/// Default requests per second.
pub const DEFAULT_REQUESTS_PER_SECOND: u32 = 1_000;

/// The default quota constructor.
#[derive(Debug, Clone, Copy)]
pub struct DefaultQuota;

impl DefaultQuota {
    /// Initializes a default [`Quota`].
    ///
    /// Uses 1,000 requests per second.
    pub const fn init() -> Quota {
        Quota::per_second(NZU32!(DEFAULT_REQUESTS_PER_SECOND))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_requests_per_second_is_1000() {
        assert_eq!(DEFAULT_REQUESTS_PER_SECOND, 1_000);
    }

    #[test]
    fn default_quota_has_debug_impl() {
        let quota = DefaultQuota;
        let debug_str = format!("{:?}", quota);
        assert!(debug_str.contains("DefaultQuota"));
    }

    #[test]
    fn default_quota_is_copy() {
        let quota = DefaultQuota;
        let quota2 = quota;
        let _ = quota;
        let _ = quota2;
    }

    #[test]
    fn quota_init_returns_valid_quota() {
        let _quota = DefaultQuota::init();
    }
}
