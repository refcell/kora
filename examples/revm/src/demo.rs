//! Deterministic demo scenario for the simulation.
//!
//! The example chain "prefunds" two addresses and injects a single transfer at height 1.

use alloy_primitives::{Address, U256};
use k256::ecdsa::SigningKey;
use kora_domain::{Tx, evm::Evm};

use crate::chain::CHAIN_ID;
#[derive(Clone, Debug)]
pub(super) struct DemoTransfer {
    pub(super) from: Address,
    pub(super) to: Address,
    pub(super) alloc: Vec<(Address, U256)>,
    pub(super) tx: Tx,
    pub(super) expected_from: U256,
    pub(super) expected_to: U256,
}

impl DemoTransfer {
    pub(super) fn new() -> Self {
        let sender = sender_key();
        let receiver = receiver_key();
        let from = Evm::address_from_key(&sender);
        let to = Evm::address_from_key(&receiver);
        let tx = Evm::sign_eip1559_transfer(&sender, CHAIN_ID, to, U256::from(100u64), 0, 21_000);

        Self {
            from,
            to,
            alloc: vec![(from, U256::from(1_000_000u64)), (to, U256::ZERO)],
            tx,
            expected_from: U256::from(1_000_000u64 - 100),
            expected_to: U256::from(100u64),
        }
    }
}

fn sender_key() -> SigningKey {
    SigningKey::from_bytes(&[1u8; 32].into()).expect("valid sender key")
}

fn receiver_key() -> SigningKey {
    SigningKey::from_bytes(&[2u8; 32].into()).expect("valid receiver key")
}
