//! Transaction validation.

use alloy_consensus::{Transaction, TxEnvelope};
use alloy_eips::eip2718::Decodable2718;
use alloy_primitives::{Address, B256, U256, keccak256};
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

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use alloy_consensus::{SignableTransaction as _, TxEip1559, TxLegacy};
    use alloy_eips::{
        eip2718::Encodable2718,
        eip2930::{AccessList, AccessListItem},
    };
    use alloy_primitives::{Bytes, Signature, TxKind};
    use k256::{ecdsa::SigningKey, elliptic_curve::rand_core::OsRng};
    use parking_lot::RwLock;

    use super::*;

    /// Mock state database for testing.
    #[derive(Clone)]
    struct MockState {
        nonces: Arc<RwLock<HashMap<Address, u64>>>,
        balances: Arc<RwLock<HashMap<Address, U256>>>,
    }

    impl MockState {
        fn new() -> Self {
            Self {
                nonces: Arc::new(RwLock::new(HashMap::new())),
                balances: Arc::new(RwLock::new(HashMap::new())),
            }
        }

        fn with_account(self, address: Address, nonce: u64, balance: U256) -> Self {
            self.nonces.write().insert(address, nonce);
            self.balances.write().insert(address, balance);
            self
        }
    }

    impl kora_traits::StateDbRead for MockState {
        async fn nonce(&self, address: &Address) -> Result<u64, kora_traits::StateDbError> {
            Ok(*self.nonces.read().get(address).unwrap_or(&0))
        }

        async fn balance(&self, address: &Address) -> Result<U256, kora_traits::StateDbError> {
            Ok(*self.balances.read().get(address).unwrap_or(&U256::ZERO))
        }

        async fn code_hash(&self, _: &Address) -> Result<B256, kora_traits::StateDbError> {
            Ok(B256::ZERO)
        }

        async fn code(&self, _: &B256) -> Result<Bytes, kora_traits::StateDbError> {
            Ok(Bytes::new())
        }

        async fn storage(&self, _: &Address, _: &U256) -> Result<U256, kora_traits::StateDbError> {
            Ok(U256::ZERO)
        }
    }

    /// Sign a transaction with a random key and return (sender, signed_envelope, raw_bytes).
    fn sign_eip1559_tx(
        chain_id: u64,
        nonce: u64,
        gas_limit: u64,
        max_fee_per_gas: u128,
        value: U256,
        to: Option<Address>,
    ) -> (Address, TxEnvelope, Tx) {
        let signing_key = SigningKey::random(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let pubkey = verifying_key.to_encoded_point(false);
        let pubkey_bytes = pubkey.as_bytes();
        let pubkey_hash = sha3::Keccak256::digest(&pubkey_bytes[1..]);
        let sender = Address::from_slice(&pubkey_hash[12..]);

        let tx = TxEip1559 {
            chain_id,
            nonce,
            gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas: max_fee_per_gas,
            to: to.map(TxKind::Call).unwrap_or(TxKind::Create),
            value,
            access_list: Default::default(),
            input: Bytes::new(),
        };

        let sig_hash = tx.signature_hash();
        let (sig, recovery_id) = signing_key.sign_prehash_recoverable(sig_hash.as_slice()).unwrap();
        let r = U256::from_be_slice(&sig.r().to_bytes());
        let s = U256::from_be_slice(&sig.s().to_bytes());
        let v = recovery_id.is_y_odd();
        let signature = Signature::new(r, s, v);

        let signed = tx.into_signed(signature);
        let envelope = TxEnvelope::from(signed);
        let mut raw_bytes = Vec::new();
        envelope.encode_2718(&mut raw_bytes);

        (sender, envelope, Tx::new(raw_bytes.into()))
    }

    fn sign_legacy_tx(
        chain_id: u64,
        nonce: u64,
        gas_limit: u64,
        gas_price: u128,
        value: U256,
        to: Option<Address>,
    ) -> (Address, TxEnvelope, Tx) {
        let signing_key = SigningKey::random(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let pubkey = verifying_key.to_encoded_point(false);
        let pubkey_bytes = pubkey.as_bytes();
        let pubkey_hash = sha3::Keccak256::digest(&pubkey_bytes[1..]);
        let sender = Address::from_slice(&pubkey_hash[12..]);

        let tx = TxLegacy {
            chain_id: Some(chain_id),
            nonce,
            gas_limit,
            gas_price,
            to: to.map(TxKind::Call).unwrap_or(TxKind::Create),
            value,
            input: Bytes::new(),
        };

        let sig_hash = tx.signature_hash();
        let (sig, recovery_id) = signing_key.sign_prehash_recoverable(sig_hash.as_slice()).unwrap();
        let r = U256::from_be_slice(&sig.r().to_bytes());
        let s = U256::from_be_slice(&sig.s().to_bytes());
        let v = recovery_id.is_y_odd();
        let signature = Signature::new(r, s, v);

        let signed = tx.into_signed(signature);
        let envelope = TxEnvelope::from(signed);
        let mut raw_bytes = Vec::new();
        envelope.encode_2718(&mut raw_bytes);

        (sender, envelope, Tx::new(raw_bytes.into()))
    }

    #[tokio::test]
    async fn validate_valid_eip1559_transaction() {
        let chain_id = 1u64;
        let (sender, _, raw_tx) = sign_eip1559_tx(
            chain_id,
            0,
            21000,
            1_000_000_000,
            U256::from(1000),
            Some(Address::ZERO),
        );

        let state =
            MockState::new().with_account(sender, 0, U256::from(1_000_000_000_000_000_000u64));
        let config = PoolConfig::default();
        let validator = TransactionValidator::new(chain_id, state, config);

        let result = validator.validate(raw_tx).await;
        assert!(result.is_ok());

        let validated = result.unwrap();
        assert_eq!(validated.sender, sender);
        assert_eq!(validated.nonce, 0);
        assert_eq!(validated.gas_limit, 21000);
    }

    #[tokio::test]
    async fn validate_valid_legacy_transaction() {
        let chain_id = 1u64;
        let (sender, _, raw_tx) = sign_legacy_tx(
            chain_id,
            0,
            21000,
            1_000_000_000,
            U256::from(1000),
            Some(Address::ZERO),
        );

        let state =
            MockState::new().with_account(sender, 0, U256::from(1_000_000_000_000_000_000u64));
        let config = PoolConfig::default();
        let validator = TransactionValidator::new(chain_id, state, config);

        let result = validator.validate(raw_tx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn reject_transaction_too_large() {
        let chain_id = 1u64;
        let (sender, _, raw_tx) =
            sign_eip1559_tx(chain_id, 0, 21000, 1_000_000_000, U256::ZERO, Some(Address::ZERO));

        let state =
            MockState::new().with_account(sender, 0, U256::from(1_000_000_000_000_000_000u64));
        let config = PoolConfig::default().with_max_tx_size(10); // Very small limit
        let validator = TransactionValidator::new(chain_id, state, config);

        let result = validator.validate(raw_tx).await;
        assert!(matches!(result, Err(TxPoolError::TxTooLarge { .. })));
    }

    #[tokio::test]
    async fn reject_invalid_chain_id() {
        let chain_id = 1u64;
        let wrong_chain_id = 5u64;
        let (sender, _, raw_tx) = sign_eip1559_tx(
            wrong_chain_id,
            0,
            21000,
            1_000_000_000,
            U256::ZERO,
            Some(Address::ZERO),
        );

        let state =
            MockState::new().with_account(sender, 0, U256::from(1_000_000_000_000_000_000u64));
        let config = PoolConfig::default();
        let validator = TransactionValidator::new(chain_id, state, config);

        let result = validator.validate(raw_tx).await;
        assert!(matches!(result, Err(TxPoolError::InvalidChainId { got: 5, expected: 1 })));
    }

    #[tokio::test]
    async fn reject_gas_price_too_low() {
        let chain_id = 1u64;
        let (sender, _, raw_tx) = sign_eip1559_tx(
            chain_id,
            0,
            21000,
            100, // Low gas price
            U256::ZERO,
            Some(Address::ZERO),
        );

        let state =
            MockState::new().with_account(sender, 0, U256::from(1_000_000_000_000_000_000u64));
        let config = PoolConfig::default().with_min_gas_price(1_000_000_000);
        let validator = TransactionValidator::new(chain_id, state, config);

        let result = validator.validate(raw_tx).await;
        assert!(matches!(result, Err(TxPoolError::GasPriceTooLow { .. })));
    }

    #[tokio::test]
    async fn reject_intrinsic_gas_too_low() {
        let chain_id = 1u64;
        let (sender, _, raw_tx) = sign_eip1559_tx(
            chain_id,
            0,
            1000, // Gas limit below intrinsic (21000)
            1_000_000_000,
            U256::ZERO,
            Some(Address::ZERO),
        );

        let state =
            MockState::new().with_account(sender, 0, U256::from(1_000_000_000_000_000_000u64));
        let config = PoolConfig::default();
        let validator = TransactionValidator::new(chain_id, state, config);

        let result = validator.validate(raw_tx).await;
        assert!(matches!(result, Err(TxPoolError::IntrinsicGasTooLow { .. })));
    }

    #[tokio::test]
    async fn reject_nonce_too_low() {
        let chain_id = 1u64;
        let (sender, _, raw_tx) = sign_eip1559_tx(
            chain_id,
            0, // nonce 0
            21000,
            1_000_000_000,
            U256::ZERO,
            Some(Address::ZERO),
        );

        let state =
            MockState::new().with_account(sender, 5, U256::from(1_000_000_000_000_000_000u64)); // State nonce is 5
        let config = PoolConfig::default();
        let validator = TransactionValidator::new(chain_id, state, config);

        let result = validator.validate(raw_tx).await;
        assert!(matches!(result, Err(TxPoolError::NonceTooLow { got: 0, expected: 5 })));
    }

    #[tokio::test]
    async fn reject_insufficient_balance() {
        let chain_id = 1u64;
        let (sender, _, raw_tx) = sign_eip1559_tx(
            chain_id,
            0,
            21000,
            1_000_000_000,                        // 1 gwei
            U256::from(1_000_000_000_000_000u64), // 0.001 ETH
            Some(Address::ZERO),
        );

        let state = MockState::new().with_account(sender, 0, U256::from(1000)); // Only 1000 wei
        let config = PoolConfig::default();
        let validator = TransactionValidator::new(chain_id, state, config);

        let result = validator.validate(raw_tx).await;
        assert!(matches!(result, Err(TxPoolError::InsufficientBalance { .. })));
    }

    #[tokio::test]
    async fn accept_future_nonce() {
        let chain_id = 1u64;
        let (sender, _, raw_tx) = sign_eip1559_tx(
            chain_id,
            10, // Future nonce
            21000,
            1_000_000_000,
            U256::ZERO,
            Some(Address::ZERO),
        );

        let state =
            MockState::new().with_account(sender, 0, U256::from(1_000_000_000_000_000_000u64));
        let config = PoolConfig::default();
        let validator = TransactionValidator::new(chain_id, state, config);

        let result = validator.validate(raw_tx).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().nonce, 10);
    }

    #[tokio::test]
    async fn validated_tx_into_ordered() {
        let chain_id = 1u64;
        let (sender, _, raw_tx) =
            sign_eip1559_tx(chain_id, 0, 21000, 1_000_000_000, U256::ZERO, Some(Address::ZERO));

        let state =
            MockState::new().with_account(sender, 0, U256::from(1_000_000_000_000_000_000u64));
        let config = PoolConfig::default();
        let validator = TransactionValidator::new(chain_id, state, config);

        let validated = validator.validate(raw_tx).await.unwrap();
        let timestamp = 1234567890u64;
        let ordered = validated.into_ordered(timestamp);

        assert_eq!(ordered.sender, sender);
        assert_eq!(ordered.nonce, 0);
        assert_eq!(ordered.timestamp, timestamp);
    }

    #[test]
    fn effective_gas_price_eip1559() {
        let tx = TxEip1559 {
            chain_id: 1,
            nonce: 0,
            gas_limit: 21000,
            max_fee_per_gas: 2_000_000_000,
            max_priority_fee_per_gas: 1_000_000_000,
            to: TxKind::Call(Address::ZERO),
            value: U256::ZERO,
            access_list: Default::default(),
            input: Bytes::new(),
        };
        let sig = Signature::from_scalars_and_parity(B256::ZERO, B256::ZERO, false);
        let signed = tx.into_signed(sig);
        let envelope = TxEnvelope::from(signed);

        assert_eq!(effective_gas_price(&envelope), 2_000_000_000);
    }

    #[test]
    fn effective_gas_price_legacy() {
        let tx = TxLegacy {
            chain_id: Some(1),
            nonce: 0,
            gas_limit: 21000,
            gas_price: 3_000_000_000,
            to: TxKind::Call(Address::ZERO),
            value: U256::ZERO,
            input: Bytes::new(),
        };
        let sig = Signature::from_scalars_and_parity(B256::ZERO, B256::ZERO, false);
        let signed = tx.into_signed(sig);
        let envelope = TxEnvelope::from(signed);

        assert_eq!(effective_gas_price(&envelope), 3_000_000_000);
    }

    #[test]
    fn intrinsic_gas_basic_transfer() {
        let tx = TxEip1559 {
            chain_id: 1,
            nonce: 0,
            gas_limit: 21000,
            max_fee_per_gas: 1_000_000_000,
            max_priority_fee_per_gas: 1_000_000_000,
            to: TxKind::Call(Address::ZERO),
            value: U256::ZERO,
            access_list: Default::default(),
            input: Bytes::new(),
        };
        let sig = Signature::from_scalars_and_parity(B256::ZERO, B256::ZERO, false);
        let signed = tx.into_signed(sig);
        let envelope = TxEnvelope::from(signed);

        assert_eq!(intrinsic_gas(&envelope), TX_BASE_GAS);
    }

    #[test]
    fn intrinsic_gas_with_data() {
        let tx = TxEip1559 {
            chain_id: 1,
            nonce: 0,
            gas_limit: 100000,
            max_fee_per_gas: 1_000_000_000,
            max_priority_fee_per_gas: 1_000_000_000,
            to: TxKind::Call(Address::ZERO),
            value: U256::ZERO,
            access_list: Default::default(),
            input: Bytes::from(vec![0u8, 1u8, 0u8, 2u8]), // 2 zero bytes, 2 non-zero
        };
        let sig = Signature::from_scalars_and_parity(B256::ZERO, B256::ZERO, false);
        let signed = tx.into_signed(sig);
        let envelope = TxEnvelope::from(signed);

        // base + 2*zero + 2*nonzero = 21000 + 2*4 + 2*16 = 21040
        assert_eq!(
            intrinsic_gas(&envelope),
            TX_BASE_GAS + 2 * TX_DATA_ZERO_GAS + 2 * TX_DATA_NON_ZERO_GAS
        );
    }

    #[test]
    fn intrinsic_gas_contract_creation() {
        let tx = TxEip1559 {
            chain_id: 1,
            nonce: 0,
            gas_limit: 100000,
            max_fee_per_gas: 1_000_000_000,
            max_priority_fee_per_gas: 1_000_000_000,
            to: TxKind::Create,
            value: U256::ZERO,
            access_list: Default::default(),
            input: Bytes::new(),
        };
        let sig = Signature::from_scalars_and_parity(B256::ZERO, B256::ZERO, false);
        let signed = tx.into_signed(sig);
        let envelope = TxEnvelope::from(signed);

        assert_eq!(intrinsic_gas(&envelope), TX_BASE_GAS + TX_CREATE_GAS);
    }

    #[test]
    fn intrinsic_gas_with_access_list() {
        let access_list = AccessList::from(vec![AccessListItem {
            address: Address::ZERO,
            storage_keys: vec![B256::ZERO, B256::ZERO],
        }]);

        let tx = TxEip1559 {
            chain_id: 1,
            nonce: 0,
            gas_limit: 100000,
            max_fee_per_gas: 1_000_000_000,
            max_priority_fee_per_gas: 1_000_000_000,
            to: TxKind::Call(Address::ZERO),
            value: U256::ZERO,
            access_list,
            input: Bytes::new(),
        };
        let sig = Signature::from_scalars_and_parity(B256::ZERO, B256::ZERO, false);
        let signed = tx.into_signed(sig);
        let envelope = TxEnvelope::from(signed);

        // base + 1 address + 2 storage keys = 21000 + 2400 + 2*1900 = 27200
        assert_eq!(
            intrinsic_gas(&envelope),
            TX_BASE_GAS + TX_ACCESS_LIST_ADDRESS_GAS + 2 * TX_ACCESS_LIST_STORAGE_KEY_GAS
        );
    }

    #[test]
    fn max_tx_cost_calculation() {
        let tx = TxEip1559 {
            chain_id: 1,
            nonce: 0,
            gas_limit: 21000,
            max_fee_per_gas: 1_000_000_000,
            max_priority_fee_per_gas: 1_000_000_000,
            to: TxKind::Call(Address::ZERO),
            value: U256::from(1_000_000_000_000_000_000u64),
            access_list: Default::default(),
            input: Bytes::new(),
        };
        let sig = Signature::from_scalars_and_parity(B256::ZERO, B256::ZERO, false);
        let signed = tx.into_signed(sig);
        let envelope = TxEnvelope::from(signed);

        // gas_limit * max_fee + value = 21000 * 1e9 + 1e12 = 21e12 + 1e12 = 22e12
        let expected = U256::from(21000u64) * U256::from(1_000_000_000u64)
            + U256::from(1_000_000_000_000_000_000u64);
        assert_eq!(max_tx_cost(&envelope), expected);
    }

    #[test]
    fn recover_sender_from_envelope_works() {
        let signing_key = SigningKey::random(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let pubkey = verifying_key.to_encoded_point(false);
        let pubkey_bytes = pubkey.as_bytes();
        let pubkey_hash = sha3::Keccak256::digest(&pubkey_bytes[1..]);
        let expected_sender = Address::from_slice(&pubkey_hash[12..]);

        let tx = TxEip1559 {
            chain_id: 1,
            nonce: 0,
            gas_limit: 21000,
            max_fee_per_gas: 1_000_000_000,
            max_priority_fee_per_gas: 1_000_000_000,
            to: TxKind::Call(Address::ZERO),
            value: U256::ZERO,
            access_list: Default::default(),
            input: Bytes::new(),
        };

        let sig_hash = tx.signature_hash();
        let (sig, recovery_id) = signing_key.sign_prehash_recoverable(sig_hash.as_slice()).unwrap();
        let r = U256::from_be_slice(&sig.r().to_bytes());
        let s = U256::from_be_slice(&sig.s().to_bytes());
        let v = recovery_id.is_y_odd();
        let signature = Signature::new(r, s, v);

        let signed = tx.into_signed(signature);
        let envelope = TxEnvelope::from(signed);

        let recovered = recover_sender_from_envelope(&envelope).unwrap();
        assert_eq!(recovered, expected_sender);
    }

    #[tokio::test]
    async fn reject_invalid_rlp() {
        let chain_id = 1u64;
        let state = MockState::new();
        let config = PoolConfig::default();
        let validator = TransactionValidator::new(chain_id, state, config);

        let invalid_tx = Tx::new(Bytes::from(vec![0xffu8; 100]));
        let result = validator.validate(invalid_tx).await;
        assert!(matches!(result, Err(TxPoolError::DecodeError(_))));
    }
}
