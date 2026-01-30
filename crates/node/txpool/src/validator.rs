//! Transaction validation.

use alloy_consensus::{Transaction, TxEnvelope};
use alloy_eips::eip2718::Decodable2718;
use alloy_primitives::{keccak256, Address, B256, U256};
use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use kora_domain::Tx;
use kora_traits::StateDbRead;
use sha3::{Digest, Keccak256};

use crate::{config::PoolConfig, error::TxPoolError, ordering::OrderedTransaction};

const TX_BASE_GAS: u64 = 21000;
const TX_DATA_ZERO_GAS: u64 = 4;
const TX_DATA_NON_ZERO_GAS: u64 = 16;
const TX_CREATE_GAS: u64 = 32000;
const TX_ACCESS_LIST_ADDRESS_GAS: u64 = 2400;
const TX_ACCESS_LIST_STORAGE_KEY_GAS: u64 = 1900;

/// A validated transaction ready for pool insertion.
#[derive(Debug, Clone)]
pub struct ValidatedTransaction {
    /// Transaction hash.
    pub hash: B256,
    /// Sender address recovered from signature.
    pub sender: Address,
    /// Transaction nonce.
    pub nonce: u64,
    /// Gas limit.
    pub gas_limit: u64,
    /// Effective gas price for ordering.
    pub effective_gas_price: u128,
    /// Maximum cost (gas * price + value).
    pub max_cost: U256,
    /// The decoded transaction envelope.
    pub envelope: TxEnvelope,
    /// Original raw transaction bytes.
    pub raw: Tx,
}

/// Validates transactions against chain state and protocol rules.
#[derive(Debug)]
pub struct TransactionValidator<S> {
    chain_id: u64,
    state: S,
    config: PoolConfig,
}

impl<S: StateDbRead> TransactionValidator<S> {
    /// Creates a new transaction validator.
    pub const fn new(chain_id: u64, state: S, config: PoolConfig) -> Self {
        Self { chain_id, state, config }
    }

    /// Validates a raw transaction.
    pub async fn validate(&self, tx: Tx) -> Result<ValidatedTransaction, TxPoolError> {
        if tx.bytes.len() > self.config.max_tx_size {
            return Err(TxPoolError::TxTooLarge {
                size: tx.bytes.len(),
                max: self.config.max_tx_size,
            });
        }

        let envelope = TxEnvelope::decode_2718(&mut tx.bytes.as_ref())
            .map_err(|e| TxPoolError::DecodeError(e.to_string()))?;

        let tx_chain_id = envelope.chain_id().unwrap_or(self.chain_id);
        if tx_chain_id != self.chain_id {
            return Err(TxPoolError::InvalidChainId { got: tx_chain_id, expected: self.chain_id });
        }

        let (sender, hash) = recover_sender_and_hash(&envelope)?;

        let effective_gas_price = effective_gas_price(&envelope);
        if effective_gas_price < self.config.min_gas_price {
            return Err(TxPoolError::GasPriceTooLow {
                price: effective_gas_price,
                min: self.config.min_gas_price,
            });
        }

        let intrinsic_gas = intrinsic_gas(&envelope);
        let gas_limit = envelope.gas_limit();
        if gas_limit < intrinsic_gas {
            return Err(TxPoolError::IntrinsicGasTooLow {
                limit: gas_limit,
                intrinsic: intrinsic_gas,
            });
        }

        let nonce = envelope.nonce();
        let state_nonce =
            self.state.nonce(&sender).await.map_err(|e| TxPoolError::StateError(e.to_string()))?;
        if nonce < state_nonce {
            return Err(TxPoolError::NonceTooLow { got: nonce, expected: state_nonce });
        }

        let max_cost = max_tx_cost(&envelope);
        let balance = self
            .state
            .balance(&sender)
            .await
            .map_err(|e| TxPoolError::StateError(e.to_string()))?;
        if balance < max_cost {
            return Err(TxPoolError::InsufficientBalance { need: max_cost, have: balance });
        }

        Ok(ValidatedTransaction {
            hash,
            sender,
            nonce,
            gas_limit,
            effective_gas_price,
            max_cost,
            envelope,
            raw: tx,
        })
    }
}

impl ValidatedTransaction {
    /// Converts this validated transaction into an ordered transaction for pool insertion.
    pub fn into_ordered(self, timestamp: u64) -> OrderedTransaction {
        OrderedTransaction::new(
            self.hash,
            self.sender,
            self.nonce,
            self.effective_gas_price,
            timestamp,
            self.envelope,
        )
    }
}

/// Recovers the sender address from a transaction envelope.
pub fn recover_sender_from_envelope(envelope: &TxEnvelope) -> Result<Address, TxPoolError> {
    recover_sender_and_hash(envelope).map(|(addr, _)| addr)
}

fn recover_sender_and_hash(envelope: &TxEnvelope) -> Result<(Address, B256), TxPoolError> {
    let hash = keccak256(alloy_rlp::encode(envelope));

    let signature = envelope.signature();
    let r_be = B256::from_slice(&signature.r().to_be_bytes::<32>());
    let s_be = B256::from_slice(&signature.s().to_be_bytes::<32>());
    let mut sig_bytes = [0u8; 64];
    sig_bytes[..32].copy_from_slice(r_be.as_slice());
    sig_bytes[32..].copy_from_slice(s_be.as_slice());

    let sig = Signature::from_slice(&sig_bytes).map_err(|_| TxPoolError::InvalidSignature)?;

    let v = signature.v();
    let v_val: u64 = if v { 1 } else { 0 };
    let recovery_id =
        RecoveryId::try_from(v_val as u8).map_err(|_| TxPoolError::InvalidSignature)?;

    let signing_hash = envelope.signature_hash();
    let verifying_key =
        VerifyingKey::recover_from_prehash(signing_hash.as_slice(), &sig, recovery_id)
            .map_err(|_| TxPoolError::InvalidSignature)?;

    let pubkey = verifying_key.to_encoded_point(false);
    let pubkey_bytes = pubkey.as_bytes();
    let pubkey_hash = Keccak256::digest(&pubkey_bytes[1..]);
    let sender = Address::from_slice(&pubkey_hash[12..]);

    Ok((sender, hash))
}

fn effective_gas_price(envelope: &TxEnvelope) -> u128 {
    match envelope {
        TxEnvelope::Legacy(tx) => tx.tx().gas_price,
        TxEnvelope::Eip2930(tx) => tx.tx().gas_price,
        TxEnvelope::Eip1559(tx) => tx.tx().max_fee_per_gas,
        TxEnvelope::Eip4844(tx) => tx.tx().tx().max_fee_per_gas,
        TxEnvelope::Eip7702(tx) => tx.tx().max_fee_per_gas,
    }
}

fn intrinsic_gas(envelope: &TxEnvelope) -> u64 {
    let mut gas = TX_BASE_GAS;

    let input = envelope.input();
    for byte in input.iter() {
        if *byte == 0 {
            gas += TX_DATA_ZERO_GAS;
        } else {
            gas += TX_DATA_NON_ZERO_GAS;
        }
    }

    if envelope.to().is_none() {
        gas += TX_CREATE_GAS;
    }

    let access_list_gas = match envelope {
        TxEnvelope::Legacy(_) => 0,
        TxEnvelope::Eip2930(tx) => access_list_gas_cost(&tx.tx().access_list),
        TxEnvelope::Eip1559(tx) => access_list_gas_cost(&tx.tx().access_list),
        TxEnvelope::Eip4844(tx) => access_list_gas_cost(&tx.tx().tx().access_list),
        TxEnvelope::Eip7702(tx) => access_list_gas_cost(&tx.tx().access_list),
    };
    gas += access_list_gas;

    gas
}

fn access_list_gas_cost(access_list: &alloy_eips::eip2930::AccessList) -> u64 {
    let mut gas = 0u64;
    for item in access_list.iter() {
        gas += TX_ACCESS_LIST_ADDRESS_GAS;
        gas += item.storage_keys.len() as u64 * TX_ACCESS_LIST_STORAGE_KEY_GAS;
    }
    gas
}

fn max_tx_cost(envelope: &TxEnvelope) -> U256 {
    let gas_limit = U256::from(envelope.gas_limit());
    let max_fee = U256::from(effective_gas_price(envelope));
    let value = envelope.value();

    gas_limit * max_fee + value
}
