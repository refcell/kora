//! Transaction pool configuration.

/// Configuration for the transaction pool.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of pending (executable) transactions.
    pub max_pending_txs: usize,
    /// Maximum number of queued (future nonce) transactions.
    pub max_queued_txs: usize,
    /// Maximum transactions allowed per sender.
    pub max_txs_per_sender: usize,
    /// Maximum transaction size in bytes.
    pub max_tx_size: usize,
    /// Minimum gas price required for transaction acceptance.
    pub min_gas_price: u128,
    /// Percentage bump required for replacement transactions.
    pub replacement_bump_percent: u8,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_pending_txs: 4096,
            max_queued_txs: 1024,
            max_txs_per_sender: 16,
            max_tx_size: 128 * 1024, // 128 KB
            min_gas_price: 0,
            replacement_bump_percent: 10,
        }
    }
}

impl PoolConfig {
    /// Creates a new pool configuration with default values.
    pub const fn new() -> Self {
        Self {
            max_pending_txs: 4096,
            max_queued_txs: 1024,
            max_txs_per_sender: 16,
            max_tx_size: 128 * 1024,
            min_gas_price: 0,
            replacement_bump_percent: 10,
        }
    }

    /// Sets the maximum number of pending transactions.
    pub const fn with_max_pending_txs(mut self, max: usize) -> Self {
        self.max_pending_txs = max;
        self
    }

    /// Sets the maximum number of queued transactions.
    pub const fn with_max_queued_txs(mut self, max: usize) -> Self {
        self.max_queued_txs = max;
        self
    }

    /// Sets the maximum transactions per sender.
    pub const fn with_max_txs_per_sender(mut self, max: usize) -> Self {
        self.max_txs_per_sender = max;
        self
    }

    /// Sets the maximum transaction size in bytes.
    pub const fn with_max_tx_size(mut self, max: usize) -> Self {
        self.max_tx_size = max;
        self
    }

    /// Sets the minimum gas price required.
    pub const fn with_min_gas_price(mut self, min: u128) -> Self {
        self.min_gas_price = min;
        self
    }

    /// Sets the replacement bump percentage.
    pub const fn with_replacement_bump_percent(mut self, percent: u8) -> Self {
        self.replacement_bump_percent = percent;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let config = PoolConfig::default();
        assert_eq!(config.max_pending_txs, 4096);
        assert_eq!(config.max_queued_txs, 1024);
        assert_eq!(config.max_txs_per_sender, 16);
        assert_eq!(config.max_tx_size, 128 * 1024);
        assert_eq!(config.min_gas_price, 0);
        assert_eq!(config.replacement_bump_percent, 10);
    }

    #[test]
    fn new_matches_default() {
        let new = PoolConfig::new();
        let default = PoolConfig::default();
        assert_eq!(new.max_pending_txs, default.max_pending_txs);
        assert_eq!(new.max_queued_txs, default.max_queued_txs);
        assert_eq!(new.max_txs_per_sender, default.max_txs_per_sender);
        assert_eq!(new.max_tx_size, default.max_tx_size);
        assert_eq!(new.min_gas_price, default.min_gas_price);
        assert_eq!(new.replacement_bump_percent, default.replacement_bump_percent);
    }

    #[test]
    fn builder_with_max_pending_txs() {
        let config = PoolConfig::new().with_max_pending_txs(8192);
        assert_eq!(config.max_pending_txs, 8192);
        assert_eq!(config.max_queued_txs, 1024);
    }

    #[test]
    fn builder_with_max_queued_txs() {
        let config = PoolConfig::new().with_max_queued_txs(2048);
        assert_eq!(config.max_queued_txs, 2048);
        assert_eq!(config.max_pending_txs, 4096);
    }

    #[test]
    fn builder_with_max_txs_per_sender() {
        let config = PoolConfig::new().with_max_txs_per_sender(32);
        assert_eq!(config.max_txs_per_sender, 32);
    }

    #[test]
    fn builder_with_max_tx_size() {
        let config = PoolConfig::new().with_max_tx_size(256 * 1024);
        assert_eq!(config.max_tx_size, 256 * 1024);
    }

    #[test]
    fn builder_with_min_gas_price() {
        let config = PoolConfig::new().with_min_gas_price(1_000_000_000);
        assert_eq!(config.min_gas_price, 1_000_000_000);
    }

    #[test]
    fn builder_with_replacement_bump_percent() {
        let config = PoolConfig::new().with_replacement_bump_percent(25);
        assert_eq!(config.replacement_bump_percent, 25);
    }

    #[test]
    fn builder_chaining() {
        let config = PoolConfig::new()
            .with_max_pending_txs(10000)
            .with_max_queued_txs(5000)
            .with_max_txs_per_sender(50)
            .with_max_tx_size(64 * 1024)
            .with_min_gas_price(500)
            .with_replacement_bump_percent(15);

        assert_eq!(config.max_pending_txs, 10000);
        assert_eq!(config.max_queued_txs, 5000);
        assert_eq!(config.max_txs_per_sender, 50);
        assert_eq!(config.max_tx_size, 64 * 1024);
        assert_eq!(config.min_gas_price, 500);
        assert_eq!(config.replacement_bump_percent, 15);
    }

    #[test]
    fn clone_preserves_values() {
        let config = PoolConfig::new().with_max_pending_txs(100).with_min_gas_price(999);
        let cloned = config.clone();

        assert_eq!(cloned.max_pending_txs, 100);
        assert_eq!(cloned.min_gas_price, 999);
    }

    #[test]
    fn debug_impl() {
        let config = PoolConfig::new();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("PoolConfig"));
        assert!(debug_str.contains("max_pending_txs"));
    }
}
