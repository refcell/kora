//! Test configuration and setup utilities.

use std::time::Duration;

use alloy_primitives::{Address, U256};
use k256::ecdsa::SigningKey;
use kora_domain::{BootstrapConfig, Tx, evm::Evm};
use kora_transport_sim::SimLinkConfig;

/// Configuration for an e2e test run.
#[derive(Clone, Debug)]
pub struct TestConfig {
    /// Number of validator nodes.
    pub validators: usize,
    /// Threshold for BLS signatures (typically n - f where f = (n-1)/3).
    pub threshold: u32,
    /// Random seed for deterministic test runs.
    pub seed: u64,
    /// Network link configuration.
    pub link: SimLinkConfig,
    /// Chain ID for EVM execution.
    pub chain_id: u64,
    /// Block gas limit.
    pub gas_limit: u64,
    /// Maximum blocks to wait for finalization.
    pub max_blocks: u64,
    /// Test timeout.
    pub timeout: Duration,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            validators: 4,
            threshold: 3,
            seed: 42,
            link: SimLinkConfig {
                latency: Duration::from_millis(10),
                jitter: Duration::from_millis(1),
                success_rate: 1.0,
            },
            chain_id: 1337,
            gas_limit: 30_000_000,
            max_blocks: 5,
            timeout: Duration::from_secs(30),
        }
    }
}

impl TestConfig {
    /// Create a new test configuration with specified validator count.
    #[must_use]
    pub const fn with_validators(mut self, n: usize) -> Self {
        self.validators = n;
        // BFT threshold: n - f where f = floor((n-1)/3)
        let f = (n.saturating_sub(1)) / 3;
        self.threshold = (n - f) as u32;
        self
    }

    /// Set random seed for reproducibility.
    #[must_use]
    pub const fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Set network link parameters.
    #[must_use]
    pub const fn with_link(mut self, link: SimLinkConfig) -> Self {
        self.link = link;
        self
    }

    /// Set maximum blocks to finalize before stopping.
    #[must_use]
    pub const fn with_max_blocks(mut self, blocks: u64) -> Self {
        self.max_blocks = blocks;
        self
    }

    /// Set test timeout.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Test scenario setup with genesis state and transactions.
#[derive(Debug, Clone)]
pub struct TestSetup {
    /// Genesis account allocations.
    pub genesis_alloc: Vec<(Address, U256)>,
    /// Bootstrap transactions to include in first block.
    pub bootstrap_txs: Vec<Tx>,
    /// Expected state after execution (for assertions).
    pub expected_balances: Vec<(Address, U256)>,
}

impl TestSetup {
    /// Create an empty test setup.
    pub const fn empty() -> Self {
        Self { genesis_alloc: Vec::new(), bootstrap_txs: Vec::new(), expected_balances: Vec::new() }
    }

    /// Create a simple transfer test setup.
    pub fn simple_transfer(chain_id: u64) -> Self {
        let sender_key = SigningKey::from_bytes(&[1u8; 32].into()).expect("valid key");
        let receiver_key = SigningKey::from_bytes(&[2u8; 32].into()).expect("valid key");
        let sender = Evm::address_from_key(&sender_key);
        let receiver = Evm::address_from_key(&receiver_key);

        let initial_balance = U256::from(1_000_000u64);
        let transfer_amount = U256::from(100u64);

        let tx =
            Evm::sign_eip1559_transfer(&sender_key, chain_id, receiver, transfer_amount, 0, 21_000);

        Self {
            genesis_alloc: vec![(sender, initial_balance), (receiver, U256::ZERO)],
            bootstrap_txs: vec![tx],
            expected_balances: vec![
                (sender, initial_balance - transfer_amount),
                (receiver, transfer_amount),
            ],
        }
    }

    /// Create a multi-transfer test setup with multiple senders.
    pub fn multi_transfer(chain_id: u64, count: usize) -> Self {
        let mut genesis_alloc = Vec::with_capacity(count * 2);
        let mut bootstrap_txs = Vec::with_capacity(count);
        let mut expected_balances = Vec::with_capacity(count * 2);

        let initial_balance = U256::from(1_000_000u64);
        let transfer_amount = U256::from(100u64);

        for i in 0..count {
            let sender_key =
                SigningKey::from_bytes(&[(i + 1) as u8; 32].into()).expect("valid key");
            let receiver_key =
                SigningKey::from_bytes(&[(i + 100) as u8; 32].into()).expect("valid key");
            let sender = Evm::address_from_key(&sender_key);
            let receiver = Evm::address_from_key(&receiver_key);

            genesis_alloc.push((sender, initial_balance));
            genesis_alloc.push((receiver, U256::ZERO));

            let tx = Evm::sign_eip1559_transfer(
                &sender_key,
                chain_id,
                receiver,
                transfer_amount,
                0,
                21_000,
            );
            bootstrap_txs.push(tx);

            expected_balances.push((sender, initial_balance - transfer_amount));
            expected_balances.push((receiver, transfer_amount));
        }

        Self { genesis_alloc, bootstrap_txs, expected_balances }
    }

    /// Create a sequential nonce test (multiple txs from same sender).
    pub fn sequential_nonces(chain_id: u64, tx_count: usize) -> Self {
        let sender_key = SigningKey::from_bytes(&[1u8; 32].into()).expect("valid key");
        let receiver_key = SigningKey::from_bytes(&[2u8; 32].into()).expect("valid key");
        let sender = Evm::address_from_key(&sender_key);
        let receiver = Evm::address_from_key(&receiver_key);

        let initial_balance = U256::from(10_000_000u64);
        let transfer_amount = U256::from(100u64);

        let mut bootstrap_txs = Vec::with_capacity(tx_count);
        for nonce in 0..tx_count {
            let tx = Evm::sign_eip1559_transfer(
                &sender_key,
                chain_id,
                receiver,
                transfer_amount,
                nonce as u64,
                21_000,
            );
            bootstrap_txs.push(tx);
        }

        let total_transferred = transfer_amount * U256::from(tx_count);

        Self {
            genesis_alloc: vec![(sender, initial_balance), (receiver, U256::ZERO)],
            bootstrap_txs,
            expected_balances: vec![
                (sender, initial_balance - total_transferred),
                (receiver, total_transferred),
            ],
        }
    }

    /// Convert to bootstrap config.
    pub fn to_bootstrap(&self) -> BootstrapConfig {
        BootstrapConfig::new(self.genesis_alloc.clone(), self.bootstrap_txs.clone())
    }
}
