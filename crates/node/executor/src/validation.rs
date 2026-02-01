//! Transaction pre-validation.

use alloy_consensus::TxEnvelope;
use alloy_eips::eip2930::AccessList;
use alloy_primitives::{Bytes, TxKind, U256};
use alloy_rlp::Decodable;
use kora_traits::StateDb;

use crate::{ExecutionConfig, ExecutionError};

/// Maximum number of blobs per transaction (EIP-4844).
pub const MAX_BLOBS_PER_TX: usize = 6;

/// Gas cost per byte of calldata (zero byte).
pub const TX_DATA_ZERO_GAS: u64 = 4;

/// Gas cost per byte of calldata (non-zero byte).
pub const TX_DATA_NON_ZERO_GAS: u64 = 16;

/// Base gas cost for a transaction.
pub const TX_BASE_GAS: u64 = 21000;

/// Gas cost for contract creation.
pub const TX_CREATE_GAS: u64 = 32000;

/// Gas cost per access list address.
pub const ACCESS_LIST_ADDRESS_GAS: u64 = 2400;

/// Gas cost per access list storage key.
pub const ACCESS_LIST_STORAGE_KEY_GAS: u64 = 1900;

/// Transaction validator for pre-execution checks.
#[derive(Clone, Debug)]
pub struct TxValidator<'a> {
    config: &'a ExecutionConfig,
    base_fee: u64,
    blob_base_fee: Option<u128>,
}

impl<'a> TxValidator<'a> {
    /// Create a new transaction validator.
    pub const fn new(config: &'a ExecutionConfig, base_fee: u64) -> Self {
        Self { config, base_fee, blob_base_fee: None }
    }

    /// Set the blob base fee for Cancun+ validation.
    #[must_use]
    pub const fn with_blob_base_fee(mut self, blob_base_fee: u128) -> Self {
        self.blob_base_fee = Some(blob_base_fee);
        self
    }

    /// Validate a transaction before execution.
    pub async fn validate<S: StateDb>(
        &self,
        tx_bytes: &Bytes,
        state: &S,
    ) -> Result<ValidatedTx, ExecutionError> {
        let envelope = TxEnvelope::decode(&mut tx_bytes.as_ref())
            .map_err(|e| ExecutionError::TxDecode(format!("{}", e)))?;

        self.validate_envelope(&envelope, state).await
    }

    /// Validate a decoded transaction envelope.
    async fn validate_envelope<S: StateDb>(
        &self,
        envelope: &TxEnvelope,
        state: &S,
    ) -> Result<ValidatedTx, ExecutionError> {
        let (
            sender,
            chain_id,
            nonce,
            gas_limit,
            max_fee,
            max_priority_fee,
            value,
            input,
            is_create,
            access_list,
        ) = match envelope {
            TxEnvelope::Legacy(signed) => {
                let tx = signed.tx();
                let sender = signed.recover_signer().map_err(|e| {
                    ExecutionError::InvalidTx(format!("failed to recover signer: {}", e))
                })?;
                (
                    sender,
                    tx.chain_id,
                    tx.nonce,
                    tx.gas_limit,
                    tx.gas_price,
                    0,
                    tx.value,
                    &tx.input,
                    matches!(tx.to, TxKind::Create),
                    None,
                )
            }
            TxEnvelope::Eip2930(signed) => {
                let tx = signed.tx();
                let sender = signed.recover_signer().map_err(|e| {
                    ExecutionError::InvalidTx(format!("failed to recover signer: {}", e))
                })?;
                (
                    sender,
                    Some(tx.chain_id),
                    tx.nonce,
                    tx.gas_limit,
                    tx.gas_price,
                    0,
                    tx.value,
                    &tx.input,
                    matches!(tx.to, TxKind::Create),
                    Some(&tx.access_list),
                )
            }
            TxEnvelope::Eip1559(signed) => {
                let tx = signed.tx();
                let sender = signed.recover_signer().map_err(|e| {
                    ExecutionError::InvalidTx(format!("failed to recover signer: {}", e))
                })?;
                (
                    sender,
                    Some(tx.chain_id),
                    tx.nonce,
                    tx.gas_limit,
                    tx.max_fee_per_gas,
                    tx.max_priority_fee_per_gas,
                    tx.value,
                    &tx.input,
                    matches!(tx.to, TxKind::Create),
                    Some(&tx.access_list),
                )
            }
            TxEnvelope::Eip4844(signed) => {
                let tx = signed.tx().tx();
                let sender = signed.recover_signer().map_err(|e| {
                    ExecutionError::InvalidTx(format!("failed to recover signer: {}", e))
                })?;

                self.validate_blob_tx_fields(&tx.blob_versioned_hashes, tx.max_fee_per_blob_gas)?;

                (
                    sender,
                    Some(tx.chain_id),
                    tx.nonce,
                    tx.gas_limit,
                    tx.max_fee_per_gas,
                    tx.max_priority_fee_per_gas,
                    tx.value,
                    &tx.input,
                    false,
                    Some(&tx.access_list),
                )
            }
            TxEnvelope::Eip7702(signed) => {
                let tx = signed.tx();
                let sender = signed.recover_signer().map_err(|e| {
                    ExecutionError::InvalidTx(format!("failed to recover signer: {}", e))
                })?;
                (
                    sender,
                    Some(tx.chain_id),
                    tx.nonce,
                    tx.gas_limit,
                    tx.max_fee_per_gas,
                    tx.max_priority_fee_per_gas,
                    tx.value,
                    &tx.input,
                    false,
                    Some(&tx.access_list),
                )
            }
        };

        if let Some(tx_chain_id) = chain_id
            && tx_chain_id != self.config.chain_id
        {
            return Err(ExecutionError::InvalidTx(format!(
                "chain ID mismatch: expected {}, got {}",
                self.config.chain_id, tx_chain_id
            )));
        }

        let intrinsic_gas = self.calculate_intrinsic_gas(input, is_create, access_list)?;
        if gas_limit < intrinsic_gas {
            return Err(ExecutionError::InvalidTx(format!(
                "gas limit {} below intrinsic gas {}",
                gas_limit, intrinsic_gas
            )));
        }

        let account_nonce = state.nonce(&sender).await?;
        if account_nonce != nonce {
            return Err(ExecutionError::InvalidTx(format!(
                "nonce mismatch: expected {}, got {}",
                account_nonce, nonce
            )));
        }

        let account_balance = state.balance(&sender).await?;
        let max_gas_cost = U256::from(gas_limit) * U256::from(max_fee);
        let required_balance = max_gas_cost + value;
        if account_balance < required_balance {
            return Err(ExecutionError::InvalidTx(format!(
                "insufficient balance: has {}, needs {}",
                account_balance, required_balance
            )));
        }

        if max_fee < u128::from(self.base_fee) {
            return Err(ExecutionError::InvalidTx(format!(
                "max fee {} below base fee {}",
                max_fee, self.base_fee
            )));
        }

        if max_priority_fee > max_fee {
            return Err(ExecutionError::InvalidTx("max priority fee exceeds max fee".to_string()));
        }

        if let Some(access_list) = access_list {
            self.validate_access_list(access_list)?;
        }

        Ok(ValidatedTx { sender, nonce, gas_limit, intrinsic_gas })
    }

    /// Calculate intrinsic gas for a transaction.
    fn calculate_intrinsic_gas(
        &self,
        input: &Bytes,
        is_create: bool,
        access_list: Option<&AccessList>,
    ) -> Result<u64, ExecutionError> {
        let mut gas = TX_BASE_GAS;

        if is_create {
            gas = gas.saturating_add(TX_CREATE_GAS);
        }

        for byte in input.iter() {
            if *byte == 0 {
                gas = gas.saturating_add(TX_DATA_ZERO_GAS);
            } else {
                gas = gas.saturating_add(TX_DATA_NON_ZERO_GAS);
            }
        }

        if let Some(access_list) = access_list {
            for item in access_list.iter() {
                gas = gas.saturating_add(ACCESS_LIST_ADDRESS_GAS);
                gas = gas.saturating_add(
                    ACCESS_LIST_STORAGE_KEY_GAS.saturating_mul(item.storage_keys.len() as u64),
                );
            }
        }

        Ok(gas)
    }

    /// Validate blob transaction specific fields.
    fn validate_blob_tx_fields(
        &self,
        blob_versioned_hashes: &[alloy_primitives::B256],
        max_fee_per_blob_gas: u128,
    ) -> Result<(), ExecutionError> {
        if blob_versioned_hashes.is_empty() {
            return Err(ExecutionError::InvalidTx(
                "blob transaction must have at least one blob".to_string(),
            ));
        }

        if blob_versioned_hashes.len() > MAX_BLOBS_PER_TX {
            return Err(ExecutionError::InvalidTx(format!(
                "blob transaction exceeds max blobs: {} > {}",
                blob_versioned_hashes.len(),
                MAX_BLOBS_PER_TX
            )));
        }

        for hash in blob_versioned_hashes {
            if hash[0] != 0x01 {
                return Err(ExecutionError::InvalidTx(format!(
                    "invalid blob version: expected 0x01, got 0x{:02x}",
                    hash[0]
                )));
            }
        }

        if let Some(blob_base_fee) = self.blob_base_fee
            && max_fee_per_blob_gas < blob_base_fee
        {
            return Err(ExecutionError::InvalidTx(format!(
                "max fee per blob gas {} below blob base fee {}",
                max_fee_per_blob_gas, blob_base_fee
            )));
        }

        Ok(())
    }

    /// Validate access list entries.
    fn validate_access_list(&self, access_list: &AccessList) -> Result<(), ExecutionError> {
        for item in access_list.iter() {
            if item.address.is_zero() {
                return Err(ExecutionError::InvalidTx(
                    "access list contains zero address".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// A validated transaction ready for execution.
#[derive(Clone, Debug)]
pub struct ValidatedTx {
    /// Transaction sender.
    pub sender: alloy_primitives::Address,
    /// Transaction nonce.
    pub nonce: u64,
    /// Gas limit.
    pub gas_limit: u64,
    /// Intrinsic gas cost.
    pub intrinsic_gas: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intrinsic_gas_simple_transfer() {
        let config = ExecutionConfig::default();
        let validator = TxValidator::new(&config, 1000);

        let gas = validator.calculate_intrinsic_gas(&Bytes::new(), false, None).unwrap();
        assert_eq!(gas, TX_BASE_GAS);
    }

    #[test]
    fn intrinsic_gas_with_data() {
        let config = ExecutionConfig::default();
        let validator = TxValidator::new(&config, 1000);

        let data = Bytes::from(vec![0, 1, 2, 0, 0, 3]);
        let gas = validator.calculate_intrinsic_gas(&data, false, None).unwrap();

        let expected = TX_BASE_GAS + (3 * TX_DATA_ZERO_GAS) + (3 * TX_DATA_NON_ZERO_GAS);
        assert_eq!(gas, expected);
    }

    #[test]
    fn intrinsic_gas_create() {
        let config = ExecutionConfig::default();
        let validator = TxValidator::new(&config, 1000);

        let gas = validator.calculate_intrinsic_gas(&Bytes::new(), true, None).unwrap();
        assert_eq!(gas, TX_BASE_GAS + TX_CREATE_GAS);
    }

    #[test]
    fn intrinsic_gas_with_access_list() {
        use alloy_eips::eip2930::AccessListItem;
        use alloy_primitives::Address;

        let config = ExecutionConfig::default();
        let validator = TxValidator::new(&config, 1000);

        let access_list = AccessList(vec![AccessListItem {
            address: Address::repeat_byte(1),
            storage_keys: vec![Default::default(), Default::default()],
        }]);

        let gas =
            validator.calculate_intrinsic_gas(&Bytes::new(), false, Some(&access_list)).unwrap();

        let expected = TX_BASE_GAS + ACCESS_LIST_ADDRESS_GAS + (2 * ACCESS_LIST_STORAGE_KEY_GAS);
        assert_eq!(gas, expected);
    }
}
