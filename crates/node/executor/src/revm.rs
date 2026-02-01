//! REVM-based block executor.

use std::collections::BTreeMap;

use alloy_consensus::Header;
use alloy_primitives::{B256, Bytes, U256, keccak256};
use kora_qmdb::{AccountUpdate, ChangeSet};
use kora_traits::StateDb;
use revm::{
    Context, ExecuteEvm, Journal, MainBuilder,
    bytecode::Bytecode,
    context::{
        block::BlockEnv,
        result::{ExecutionResult, Output},
    },
    context_interface::{
        ContextSetters,
        transaction::{AccessList, AccessListItem},
    },
    database::State,
    primitives::{TxKind, hardfork::SpecId},
    state::{EvmState, EvmStorageSlot},
};

use crate::{
    BlockContext, BlockExecutor, ExecutionConfig, ExecutionError, ExecutionOutcome,
    ExecutionReceipt, ParentBlock, StateDbAdapter,
};

/// REVM-based block executor.
///
/// This executor uses REVM to execute EVM transactions against a state database.
/// The actual EVM execution is performed via the REVM handler traits.
#[derive(Clone, Debug)]
pub struct RevmExecutor {
    /// Execution configuration.
    config: ExecutionConfig,
}

impl RevmExecutor {
    /// Create a new REVM executor with the given chain ID.
    #[must_use]
    pub const fn new(chain_id: u64) -> Self {
        Self { config: ExecutionConfig::new(chain_id) }
    }

    /// Create a new REVM executor with full configuration.
    #[must_use]
    pub const fn with_config(config: ExecutionConfig) -> Self {
        Self { config }
    }

    /// Get the chain ID.
    pub const fn chain_id(&self) -> u64 {
        self.config.chain_id
    }

    /// Get the execution configuration.
    pub const fn config(&self) -> &ExecutionConfig {
        &self.config
    }

    /// Get the spec ID.
    pub const fn spec_id(&self) -> SpecId {
        self.config.spec_id
    }

    /// Validate a header against its parent.
    pub fn validate_header_against_parent(
        &self,
        header: &Header,
        parent: &ParentBlock,
    ) -> Result<(), ExecutionError> {
        if header.number != parent.number + 1 {
            return Err(ExecutionError::BlockValidation(format!(
                "block number not sequential: expected {}, got {}",
                parent.number + 1,
                header.number
            )));
        }

        if header.parent_hash != parent.hash {
            return Err(ExecutionError::BlockValidation(format!(
                "parent hash mismatch: expected {}, got {}",
                parent.hash, header.parent_hash
            )));
        }

        if header.timestamp <= parent.timestamp {
            return Err(ExecutionError::BlockValidation(format!(
                "timestamp not increasing: parent {}, current {}",
                parent.timestamp, header.timestamp
            )));
        }

        self.validate_gas_limit(header.gas_limit, parent.gas_limit)?;

        if let Some(parent_base_fee) = parent.base_fee_per_gas {
            self.validate_base_fee(header, parent_base_fee, parent.gas_used, parent.gas_limit)?;
        }

        Ok(())
    }

    fn validate_gas_limit(
        &self,
        gas_limit: u64,
        parent_gas_limit: u64,
    ) -> Result<(), ExecutionError> {
        let bounds = &self.config.gas_limit_bounds;

        if gas_limit < bounds.min {
            return Err(ExecutionError::BlockValidation(format!(
                "gas limit {} below minimum {}",
                gas_limit, bounds.min
            )));
        }

        if gas_limit > bounds.max {
            return Err(ExecutionError::BlockValidation(format!(
                "gas limit {} above maximum {}",
                gas_limit, bounds.max
            )));
        }

        let max_delta = parent_gas_limit / bounds.max_delta_divisor;
        let diff = gas_limit.abs_diff(parent_gas_limit);

        if diff >= max_delta {
            return Err(ExecutionError::BlockValidation(format!(
                "gas limit change {} exceeds maximum delta {}",
                diff, max_delta
            )));
        }

        Ok(())
    }

    fn validate_base_fee(
        &self,
        header: &Header,
        parent_base_fee: u64,
        parent_gas_used: u64,
        parent_gas_limit: u64,
    ) -> Result<(), ExecutionError> {
        let expected = calculate_base_fee(
            parent_base_fee,
            parent_gas_used,
            parent_gas_limit,
            &self.config.base_fee_params,
        );

        let actual = header.base_fee_per_gas.ok_or_else(|| {
            ExecutionError::BlockValidation("missing base fee in EIP-1559 block".to_string())
        })?;

        if actual != expected {
            return Err(ExecutionError::BlockValidation(format!(
                "base fee mismatch: expected {}, got {}",
                expected, actual
            )));
        }

        Ok(())
    }
}

impl Default for RevmExecutor {
    fn default() -> Self {
        Self::new(1)
    }
}

/// Calculate the expected base fee for the next block (EIP-1559).
pub fn calculate_base_fee(
    parent_base_fee: u64,
    parent_gas_used: u64,
    parent_gas_limit: u64,
    params: &crate::BaseFeeParams,
) -> u64 {
    let parent_gas_target = parent_gas_limit / params.elasticity_multiplier;

    if parent_gas_used == parent_gas_target {
        return parent_base_fee;
    }

    if parent_gas_used > parent_gas_target {
        let gas_used_delta = parent_gas_used - parent_gas_target;
        let base_fee_delta = (parent_base_fee as u128).saturating_mul(gas_used_delta as u128)
            / (parent_gas_target as u128)
            / (params.max_change_denominator as u128);
        let base_fee_delta = base_fee_delta.max(1) as u64;
        parent_base_fee.saturating_add(base_fee_delta)
    } else {
        let gas_used_delta = parent_gas_target - parent_gas_used;
        let base_fee_delta = (parent_base_fee as u128).saturating_mul(gas_used_delta as u128)
            / (parent_gas_target as u128)
            / (params.max_change_denominator as u128);
        parent_base_fee.saturating_sub(base_fee_delta as u64)
    }
}

impl<S: StateDb> BlockExecutor<S> for RevmExecutor {
    type Tx = Bytes;

    fn execute(
        &self,
        state: &S,
        context: &BlockContext,
        txs: &[Self::Tx],
    ) -> Result<ExecutionOutcome, ExecutionError> {
        let adapter = StateDbAdapter::new(state.clone());

        let db = State::builder().with_database_ref(adapter).build();

        type Db<S> = State<revm::database::WrapDatabaseRef<StateDbAdapter<S>>>;
        let ctx: Context<BlockEnv, _, _, Db<S>, Journal<Db<S>>, ()> =
            Context::new(db, self.config.spec_id);
        let ctx = ctx
            .modify_cfg_chained(|cfg| {
                cfg.chain_id = self.config.chain_id;
            })
            .modify_block_chained(|blk: &mut BlockEnv| {
                blk.number = U256::from(context.header.number);
                blk.timestamp = U256::from(context.header.timestamp);
                blk.beneficiary = context.header.beneficiary;
                blk.gas_limit = context.header.gas_limit;
                blk.basefee = context.header.base_fee_per_gas.unwrap_or_default();
                blk.prevrandao = Some(context.prevrandao);
            });

        let mut evm = ctx.build_mainnet();

        let mut outcome = ExecutionOutcome::new();
        let mut cumulative_gas = 0u64;

        for tx_bytes in txs {
            let tx_hash = keccak256(tx_bytes);

            let tx_env = decode_tx_env(tx_bytes, self.config.chain_id)?;
            evm.set_tx(tx_env);

            let result_and_state =
                evm.replay().map_err(|e| ExecutionError::TxExecution(format!("{:?}", e)))?;

            let gas_used = result_and_state.result.gas_used();
            cumulative_gas = cumulative_gas.saturating_add(gas_used);

            let receipt =
                build_receipt(&result_and_state.result, tx_hash, gas_used, cumulative_gas);
            outcome.receipts.push(receipt);

            let changes = extract_changes(result_and_state.state);
            outcome.changes.merge(changes);
        }

        outcome.gas_used = cumulative_gas;
        Ok(outcome)
    }

    fn validate_header(&self, header: &Header) -> Result<(), ExecutionError> {
        if header.gas_limit < self.config.gas_limit_bounds.min {
            return Err(ExecutionError::BlockValidation(format!(
                "gas limit {} below minimum {}",
                header.gas_limit, self.config.gas_limit_bounds.min
            )));
        }

        if header.gas_limit > self.config.gas_limit_bounds.max {
            return Err(ExecutionError::BlockValidation(format!(
                "gas limit {} above maximum {}",
                header.gas_limit, self.config.gas_limit_bounds.max
            )));
        }

        Ok(())
    }
}

/// Decode transaction bytes into a REVM TxEnv.
///
/// Currently supports basic transaction decoding for all Ethereum transaction types.
fn decode_tx_env(tx_bytes: &Bytes, _chain_id: u64) -> Result<revm::context::TxEnv, ExecutionError> {
    use alloy_consensus::TxEnvelope;
    use alloy_rlp::Decodable;

    // Decode the transaction envelope
    let envelope = TxEnvelope::decode(&mut tx_bytes.as_ref())
        .map_err(|e| ExecutionError::TxDecode(format!("{}", e)))?;

    // Build TxEnv using the builder pattern
    let mut builder = revm::context::TxEnv::builder();

    match &envelope {
        TxEnvelope::Legacy(signed) => {
            let tx = signed.tx();
            let caller = signed.recover_signer().map_err(|e| {
                ExecutionError::TxDecode(format!("failed to recover signer: {}", e))
            })?;

            builder = builder
                .caller(caller)
                .gas_limit(tx.gas_limit)
                .gas_price(tx.gas_price)
                .value(tx.value)
                .data(tx.input.clone())
                .nonce(tx.nonce)
                .chain_id(tx.chain_id)
                .kind(convert_tx_kind(tx.to));
        }
        TxEnvelope::Eip2930(signed) => {
            let tx = signed.tx();
            let caller = signed.recover_signer().map_err(|e| {
                ExecutionError::TxDecode(format!("failed to recover signer: {}", e))
            })?;

            builder = builder
                .caller(caller)
                .gas_limit(tx.gas_limit)
                .gas_price(tx.gas_price)
                .value(tx.value)
                .data(tx.input.clone())
                .nonce(tx.nonce)
                .chain_id(Some(tx.chain_id))
                .kind(convert_tx_kind(tx.to))
                .access_list(convert_access_list(&tx.access_list));
        }
        TxEnvelope::Eip1559(signed) => {
            let tx = signed.tx();
            let caller = signed.recover_signer().map_err(|e| {
                ExecutionError::TxDecode(format!("failed to recover signer: {}", e))
            })?;

            builder = builder
                .caller(caller)
                .gas_limit(tx.gas_limit)
                .gas_price(tx.max_fee_per_gas)
                .gas_priority_fee(Some(tx.max_priority_fee_per_gas))
                .value(tx.value)
                .data(tx.input.clone())
                .nonce(tx.nonce)
                .chain_id(Some(tx.chain_id))
                .kind(convert_tx_kind(tx.to))
                .access_list(convert_access_list(&tx.access_list));
        }
        TxEnvelope::Eip4844(signed) => {
            let tx = signed.tx().tx();
            let caller = signed.recover_signer().map_err(|e| {
                ExecutionError::TxDecode(format!("failed to recover signer: {}", e))
            })?;

            builder = builder
                .caller(caller)
                .gas_limit(tx.gas_limit)
                .gas_price(tx.max_fee_per_gas)
                .gas_priority_fee(Some(tx.max_priority_fee_per_gas))
                .value(tx.value)
                .data(tx.input.clone())
                .nonce(tx.nonce)
                .chain_id(Some(tx.chain_id))
                .kind(TxKind::Call(tx.to))
                .access_list(convert_access_list(&tx.access_list))
                .max_fee_per_blob_gas(tx.max_fee_per_blob_gas)
                .blob_hashes(tx.blob_versioned_hashes.clone());
        }
        TxEnvelope::Eip7702(signed) => {
            let tx = signed.tx();
            let caller = signed.recover_signer().map_err(|e| {
                ExecutionError::TxDecode(format!("failed to recover signer: {}", e))
            })?;

            builder = builder
                .caller(caller)
                .gas_limit(tx.gas_limit)
                .gas_price(tx.max_fee_per_gas)
                .gas_priority_fee(Some(tx.max_priority_fee_per_gas))
                .value(tx.value)
                .data(tx.input.clone())
                .nonce(tx.nonce)
                .chain_id(Some(tx.chain_id))
                .kind(TxKind::Call(tx.to))
                .access_list(convert_access_list(&tx.access_list))
                .authorization_list(convert_authorization_list(&tx.authorization_list));
        }
    }

    builder
        .build()
        .map_err(|e| ExecutionError::TxDecode(format!("failed to build tx env: {:?}", e)))
}

/// Convert alloy TxKind to revm TxKind.
const fn convert_tx_kind(kind: alloy_primitives::TxKind) -> TxKind {
    match kind {
        alloy_primitives::TxKind::Call(addr) => TxKind::Call(addr),
        alloy_primitives::TxKind::Create => TxKind::Create,
    }
}

/// Convert alloy AccessList to revm AccessList.
fn convert_access_list(access_list: &alloy_eips::eip2930::AccessList) -> AccessList {
    AccessList(
        access_list
            .iter()
            .map(|item| AccessListItem {
                address: item.address,
                storage_keys: item.storage_keys.clone(),
            })
            .collect(),
    )
}

/// Convert alloy authorization list to revm authorization list.
fn convert_authorization_list(
    auth_list: &[alloy_eips::eip7702::SignedAuthorization],
) -> Vec<
    revm::context_interface::either::Either<
        revm::context_interface::transaction::SignedAuthorization,
        revm::context_interface::transaction::RecoveredAuthorization,
    >,
> {
    use alloy_eips::eip7702::RecoveredAuthority;

    auth_list
        .iter()
        .map(|auth| {
            // Build the inner authorization
            let inner = revm::context_interface::transaction::Authorization {
                chain_id: *auth.chain_id(),
                address: *auth.address(),
                nonce: auth.nonce(),
            };

            // Convert to recovered authorization - use Valid if recovery succeeds, Invalid otherwise
            let recovered_authority = auth
                .recover_authority()
                .map_or(RecoveredAuthority::Invalid, RecoveredAuthority::Valid);

            revm::context_interface::either::Either::Right(
                revm::context_interface::transaction::RecoveredAuthorization::new_unchecked(
                    inner,
                    recovered_authority,
                ),
            )
        })
        .collect()
}

/// Build a transaction receipt from execution result.
fn build_receipt(
    result: &ExecutionResult,
    tx_hash: B256,
    gas_used: u64,
    cumulative_gas_used: u64,
) -> ExecutionReceipt {
    let (success, logs, contract_address) = match result {
        ExecutionResult::Success { logs, output, .. } => {
            let contract_addr = match output {
                Output::Create(_, addr) => *addr,
                Output::Call(_) => None,
            };
            // REVM logs are already alloy_primitives::Log, just clone them
            (true, logs.clone(), contract_addr)
        }
        ExecutionResult::Revert { .. } => (false, Vec::new(), None),
        ExecutionResult::Halt { .. } => (false, Vec::new(), None),
    };

    ExecutionReceipt::new(tx_hash, success, gas_used, cumulative_gas_used, logs, contract_address)
}

/// Extract state changes from REVM execution state.
fn extract_changes(state: EvmState) -> ChangeSet {
    let mut changes = ChangeSet::new();

    for (address, account) in state {
        // Skip untouched accounts
        if !account.is_touched() {
            continue;
        }

        // Extract storage changes
        let storage: BTreeMap<U256, U256> = account
            .storage
            .iter()
            .map(|(k, v): (&U256, &EvmStorageSlot)| (*k, v.present_value()))
            .collect();

        // Extract code if present
        let code = account.info.code.as_ref().map(|c: &Bytecode| c.bytes().to_vec());

        let update = AccountUpdate {
            created: account.is_created(),
            selfdestructed: account.is_selfdestructed(),
            nonce: account.info.nonce,
            balance: account.info.balance,
            code_hash: account.info.code_hash,
            code,
            storage,
        };

        changes.insert(address, update);
    }

    changes
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, Bytes, KECCAK256_EMPTY};
    use kora_qmdb::ChangeSet;
    use kora_traits::{StateDb, StateDbError, StateDbRead, StateDbWrite};
    use revm::state::Account;

    use super::*;
    use crate::GasLimitBounds;

    #[derive(Clone, Debug, Default)]
    struct MockStateDb;

    impl StateDbRead for MockStateDb {
        async fn nonce(&self, _address: &Address) -> Result<u64, StateDbError> {
            Ok(0)
        }
        async fn balance(&self, _address: &Address) -> Result<U256, StateDbError> {
            Ok(U256::ZERO)
        }
        async fn code_hash(&self, _address: &Address) -> Result<B256, StateDbError> {
            Ok(KECCAK256_EMPTY)
        }
        async fn code(&self, _code_hash: &B256) -> Result<Bytes, StateDbError> {
            Ok(Bytes::new())
        }
        async fn storage(&self, _address: &Address, _slot: &U256) -> Result<U256, StateDbError> {
            Ok(U256::ZERO)
        }
    }

    impl StateDbWrite for MockStateDb {
        async fn commit(&self, _changes: ChangeSet) -> Result<B256, StateDbError> {
            Ok(B256::ZERO)
        }
        async fn compute_root(&self, _changes: &ChangeSet) -> Result<B256, StateDbError> {
            Ok(B256::ZERO)
        }
        fn merge_changes(&self, _older: ChangeSet, newer: ChangeSet) -> ChangeSet {
            newer
        }
    }

    impl StateDb for MockStateDb {
        async fn state_root(&self) -> Result<B256, StateDbError> {
            Ok(B256::ZERO)
        }
    }

    #[test]
    fn revm_executor_new() {
        let executor = RevmExecutor::new(1);
        assert_eq!(executor.chain_id(), 1);
    }

    #[test]
    fn revm_executor_default() {
        let executor = RevmExecutor::default();
        assert_eq!(executor.chain_id(), 1);
    }

    #[test]
    fn revm_executor_with_config() {
        let config = ExecutionConfig::new(42).with_spec_id(SpecId::PRAGUE);
        let executor = RevmExecutor::with_config(config);
        assert_eq!(executor.chain_id(), 42);
        assert_eq!(executor.spec_id(), SpecId::PRAGUE);
    }

    #[test]
    fn validate_header_gas_limit_bounds() {
        let executor = RevmExecutor::with_config(ExecutionConfig::new(1).with_gas_limit_bounds(
            GasLimitBounds { min: 5000, max: 30_000_000, max_delta_divisor: 1024 },
        ));

        let mut header = Header::default();
        header.gas_limit = 1000;
        assert!(
            <RevmExecutor as BlockExecutor<MockStateDb>>::validate_header(&executor, &header)
                .is_err()
        );

        header.gas_limit = 100_000_000;
        assert!(
            <RevmExecutor as BlockExecutor<MockStateDb>>::validate_header(&executor, &header)
                .is_err()
        );

        header.gas_limit = 15_000_000;
        assert!(
            <RevmExecutor as BlockExecutor<MockStateDb>>::validate_header(&executor, &header)
                .is_ok()
        );
    }

    #[test]
    fn validate_header_against_parent_sequential() {
        let executor = RevmExecutor::new(1);

        let parent = ParentBlock {
            hash: B256::repeat_byte(1),
            number: 100,
            timestamp: 1000,
            gas_limit: 30_000_000,
            gas_used: 15_000_000,
            base_fee_per_gas: None,
        };

        let mut header = Header::default();
        header.parent_hash = B256::repeat_byte(1);
        header.number = 101;
        header.timestamp = 1001;
        header.gas_limit = 30_000_000;

        assert!(executor.validate_header_against_parent(&header, &parent).is_ok());

        header.number = 103;
        assert!(executor.validate_header_against_parent(&header, &parent).is_err());
    }

    #[test]
    fn validate_header_against_parent_timestamp() {
        let executor = RevmExecutor::new(1);

        let parent = ParentBlock {
            hash: B256::repeat_byte(1),
            number: 100,
            timestamp: 1000,
            gas_limit: 30_000_000,
            gas_used: 15_000_000,
            base_fee_per_gas: None,
        };

        let mut header = Header::default();
        header.parent_hash = B256::repeat_byte(1);
        header.number = 101;
        header.timestamp = 999;
        header.gas_limit = 30_000_000;

        assert!(executor.validate_header_against_parent(&header, &parent).is_err());
    }

    #[test]
    fn validate_header_against_parent_gas_limit_delta() {
        let executor = RevmExecutor::new(1);

        let parent = ParentBlock {
            hash: B256::repeat_byte(1),
            number: 100,
            timestamp: 1000,
            gas_limit: 30_000_000,
            gas_used: 15_000_000,
            base_fee_per_gas: None,
        };

        let mut header = Header::default();
        header.parent_hash = B256::repeat_byte(1);
        header.number = 101;
        header.timestamp = 1001;
        header.gas_limit = 35_000_000;

        assert!(executor.validate_header_against_parent(&header, &parent).is_err());
    }

    #[test]
    fn calculate_base_fee_at_target() {
        let params = crate::BaseFeeParams::default();
        let base_fee = calculate_base_fee(1000, 15_000_000, 30_000_000, &params);
        assert_eq!(base_fee, 1000);
    }

    #[test]
    fn calculate_base_fee_above_target() {
        let params = crate::BaseFeeParams::default();
        let base_fee = calculate_base_fee(1000, 20_000_000, 30_000_000, &params);
        assert!(base_fee > 1000);
    }

    #[test]
    fn calculate_base_fee_below_target() {
        let params = crate::BaseFeeParams::default();
        let base_fee = calculate_base_fee(1000, 10_000_000, 30_000_000, &params);
        assert!(base_fee < 1000);
    }

    #[test]
    fn build_receipt_success() {
        let result = ExecutionResult::Success {
            reason: revm::context::result::SuccessReason::Stop,
            gas_used: 21000,
            gas_refunded: 0,
            logs: vec![],
            output: Output::Call(Bytes::new()),
        };

        let receipt = build_receipt(&result, B256::ZERO, 21000, 21000);
        assert!(receipt.success());
        assert_eq!(receipt.gas_used, 21000);
        assert_eq!(receipt.cumulative_gas_used(), 21000);
        assert!(receipt.logs().is_empty());
        assert!(receipt.contract_address.is_none());
    }

    #[test]
    fn build_receipt_revert() {
        let result = ExecutionResult::Revert { gas_used: 21000, output: Bytes::new() };

        let receipt = build_receipt(&result, B256::ZERO, 21000, 21000);
        assert!(!receipt.success());
        assert_eq!(receipt.gas_used, 21000);
    }

    #[test]
    fn build_receipt_halt() {
        let result = ExecutionResult::Halt {
            reason: revm::context::result::HaltReason::OutOfGas(
                revm::context::result::OutOfGasError::Basic,
            ),
            gas_used: 21000,
        };

        let receipt = build_receipt(&result, B256::ZERO, 21000, 21000);
        assert!(!receipt.success());
        assert_eq!(receipt.gas_used, 21000);
    }

    #[test]
    fn extract_changes_empty() {
        let state = EvmState::default();
        let changes = extract_changes(state);
        assert!(changes.is_empty());
    }

    #[test]
    fn extract_changes_touched_account() {
        use revm::state::AccountStatus;

        let mut state = EvmState::default();

        let mut account = Account::default();
        account.info.nonce = 1;
        account.info.balance = U256::from(1000);
        account.info.code_hash = KECCAK256_EMPTY;
        account.status = AccountStatus::Touched;

        // Add a storage change
        account
            .storage
            .insert(U256::from(1), EvmStorageSlot::new_changed(U256::ZERO, U256::from(42), 0));

        state.insert(Address::ZERO, account);

        let changes = extract_changes(state);
        assert_eq!(changes.len(), 1);

        let update = changes.accounts.get(&Address::ZERO).unwrap();
        assert_eq!(update.nonce, 1);
        assert_eq!(update.balance, U256::from(1000));
        assert_eq!(update.storage.get(&U256::from(1)), Some(&U256::from(42)));
    }

    #[test]
    fn extract_changes_untouched_skipped() {
        use revm::state::AccountStatus;

        let mut state = EvmState::default();

        let mut account = Account::default();
        account.info.nonce = 1;
        account.info.balance = U256::from(1000);
        account.status = AccountStatus::empty(); // Not touched

        state.insert(Address::ZERO, account);

        let changes = extract_changes(state);
        assert!(changes.is_empty());
    }

    #[test]
    fn extract_changes_created_account() {
        use revm::state::AccountStatus;

        let mut state = EvmState::default();

        // Created accounts also need to be touched to be processed
        let account = Account {
            status: AccountStatus::Created | AccountStatus::Touched,
            ..Default::default()
        };

        state.insert(Address::ZERO, account);

        let changes = extract_changes(state);
        assert_eq!(changes.len(), 1);

        let update = changes.accounts.get(&Address::ZERO).unwrap();
        assert!(update.created);
    }

    #[test]
    fn extract_changes_selfdestructed() {
        use revm::state::AccountStatus;

        let mut state = EvmState::default();

        let mut account = Account::default();
        account.info.nonce = 5;
        account.info.balance = U256::from(100);
        // SelfDestructed accounts also need to be touched to be processed
        account.status = AccountStatus::SelfDestructed | AccountStatus::Touched;

        state.insert(Address::ZERO, account);

        let changes = extract_changes(state);
        assert_eq!(changes.len(), 1);

        let update = changes.accounts.get(&Address::ZERO).unwrap();
        assert!(update.selfdestructed);
    }
}
