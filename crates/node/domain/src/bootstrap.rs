//! Boostrap configuration.

use std::{path::Path, str::FromStr};

use alloy_evm::revm::primitives::{Address, U256};
use serde::{Deserialize, Serialize};

use crate::Tx;

/// Bootstrap configuration for genesis state and initial transactions.
#[derive(Clone, Debug)]
pub struct BootstrapConfig {
    /// Initial account allocations (address, balance) for genesis.
    pub genesis_alloc: Vec<(Address, U256)>,
    /// Transactions to execute during bootstrap.
    pub bootstrap_txs: Vec<Tx>,
}

#[derive(Serialize, Deserialize)]
struct GenesisJson {
    chain_id: u64,
    timestamp: u64,
    allocations: Vec<AllocationJson>,
}

#[derive(Serialize, Deserialize)]
struct AllocationJson {
    address: String,
    balance: String,
}

impl BootstrapConfig {
    /// Create a new bootstrap configuration.
    #[must_use]
    pub const fn new(genesis_alloc: Vec<(Address, U256)>, bootstrap_txs: Vec<Tx>) -> Self {
        Self { genesis_alloc, bootstrap_txs }
    }

    /// Load bootstrap configuration from a genesis JSON file.
    pub fn load(genesis_path: &Path) -> Result<Self, BootstrapError> {
        let content = std::fs::read_to_string(genesis_path)?;
        let genesis: GenesisJson = serde_json::from_str(&content)?;

        let mut genesis_alloc = Vec::with_capacity(genesis.allocations.len());
        for alloc in genesis.allocations {
            let address = Address::from_str(&alloc.address)
                .map_err(|e| BootstrapError::Parse(format!("invalid address: {}", e)))?;
            let balance = U256::from_str(&alloc.balance)
                .map_err(|e| BootstrapError::Parse(format!("invalid balance: {}", e)))?;
            genesis_alloc.push((address, balance));
        }

        Ok(Self { genesis_alloc, bootstrap_txs: Vec::new() })
    }
}

/// Errors that can occur during bootstrap configuration loading.
#[derive(Debug)]
pub enum BootstrapError {
    /// IO error reading the genesis file.
    Io(std::io::Error),
    /// JSON parsing error.
    Json(serde_json::Error),
    /// Error parsing address or balance values.
    Parse(String),
}

impl std::fmt::Display for BootstrapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {}", e),
            Self::Json(e) => write!(f, "json error: {}", e),
            Self::Parse(e) => write!(f, "parse error: {}", e),
        }
    }
}

impl std::error::Error for BootstrapError {}

impl From<std::io::Error> for BootstrapError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for BootstrapError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
