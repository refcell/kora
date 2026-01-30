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
}
