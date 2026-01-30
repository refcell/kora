//! Integration tests for kora-executor.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use alloy_consensus::Header;
use alloy_primitives::{Address, B256, Bytes, U256};
use kora_executor::{BlockContext, BlockExecutor, RevmExecutor};
use kora_qmdb::{AccountUpdate, ChangeSet};
use kora_traits::{StateDb, StateDbError, StateDbRead, StateDbWrite};
use rstest::rstest;

/// Account data stored in the mock state database.
#[derive(Clone, Debug, Default)]
struct MockAccount {
    nonce: u64,
    balance: U256,
    code_hash: B256,
    storage: HashMap<U256, U256>,
}

/// Mock state database for testing.
///
/// Stores account state in memory using a HashMap.
#[derive(Clone, Debug, Default)]
struct MockStateDb {
    /// Accounts indexed by address.
    accounts: Arc<RwLock<HashMap<Address, MockAccount>>>,
    /// Contract code indexed by code hash.
    code: Arc<RwLock<HashMap<B256, Bytes>>>,
    /// Current state root.
    state_root: Arc<RwLock<B256>>,
}

impl MockStateDb {
    /// Create a new empty mock state database.
    fn new() -> Self {
        Self::default()
    }

    /// Insert an account into the database.
    fn insert_account(&self, address: Address, account: MockAccount) {
        self.accounts.write().unwrap().insert(address, account);
    }

    /// Insert code into the database.
    fn insert_code(&self, code_hash: B256, code: Bytes) {
        self.code.write().unwrap().insert(code_hash, code);
    }
}

impl StateDbRead for MockStateDb {
    async fn nonce(&self, address: &Address) -> Result<u64, StateDbError> {
        self.accounts
            .read()
            .unwrap()
            .get(address)
            .map(|acc| acc.nonce)
            .ok_or(StateDbError::AccountNotFound(*address))
    }

    async fn balance(&self, address: &Address) -> Result<U256, StateDbError> {
        self.accounts
            .read()
            .unwrap()
            .get(address)
            .map(|acc| acc.balance)
            .ok_or(StateDbError::AccountNotFound(*address))
    }

    async fn code_hash(&self, address: &Address) -> Result<B256, StateDbError> {
        self.accounts
            .read()
            .unwrap()
            .get(address)
            .map(|acc| acc.code_hash)
            .ok_or(StateDbError::AccountNotFound(*address))
    }

    async fn code(&self, code_hash: &B256) -> Result<Bytes, StateDbError> {
        self.code
            .read()
            .unwrap()
            .get(code_hash)
            .cloned()
            .ok_or(StateDbError::CodeNotFound(*code_hash))
    }

    async fn storage(&self, address: &Address, slot: &U256) -> Result<U256, StateDbError> {
        let accounts = self.accounts.read().unwrap();
        Ok(accounts
            .get(address)
            .and_then(|acc| acc.storage.get(slot).copied())
            .unwrap_or(U256::ZERO))
    }
}

impl StateDbWrite for MockStateDb {
    async fn commit(&self, changes: ChangeSet) -> Result<B256, StateDbError> {
        let mut accounts = self.accounts.write().unwrap();
        let mut code_store = self.code.write().unwrap();

        for (address, update) in changes.accounts {
            if update.selfdestructed {
                accounts.remove(&address);
                continue;
            }

            let account = accounts.entry(address).or_default();
            account.nonce = update.nonce;
            account.balance = update.balance;
            account.code_hash = update.code_hash;

            if let Some(code) = update.code {
                code_store.insert(update.code_hash, Bytes::from(code));
            }

            for (slot, value) in update.storage {
                if value.is_zero() {
                    account.storage.remove(&slot);
                } else {
                    account.storage.insert(slot, value);
                }
            }
        }

        // Generate a simple state root from account count.
        let root = B256::from_slice(&[accounts.len() as u8; 32]);
        *self.state_root.write().unwrap() = root;
        Ok(root)
    }

    async fn compute_root(&self, _changes: &ChangeSet) -> Result<B256, StateDbError> {
        // Simplified: just return current state root.
        Ok(*self.state_root.read().unwrap())
    }

    fn merge_changes(&self, mut older: ChangeSet, newer: ChangeSet) -> ChangeSet {
        older.merge(newer);
        older
    }
}

impl StateDb for MockStateDb {
    async fn state_root(&self) -> Result<B256, StateDbError> {
        Ok(*self.state_root.read().unwrap())
    }
}

// ----------------------------------------------------------------------------
// Tests for RevmExecutor creation with different chain IDs
// ----------------------------------------------------------------------------

#[rstest]
#[case(1, "Ethereum Mainnet")]
#[case(11155111, "Sepolia")]
#[case(42161, "Arbitrum One")]
#[case(10, "Optimism")]
#[case(137, "Polygon")]
#[case(u64::MAX, "Max chain ID")]
fn test_revm_executor_chain_ids(#[case] chain_id: u64, #[case] _name: &str) {
    let executor = RevmExecutor::new(chain_id);
    assert_eq!(executor.chain_id(), chain_id);
}

#[test]
fn test_revm_executor_default_chain_id() {
    let executor = RevmExecutor::default();
    assert_eq!(executor.chain_id(), 1);
}

// ----------------------------------------------------------------------------
// Tests for execute with empty transaction list
// ----------------------------------------------------------------------------

#[test]
fn test_execute_empty_transactions_returns_empty_outcome() {
    let executor = RevmExecutor::new(1);
    let state = MockStateDb::new();
    let context = BlockContext::new(Header::default(), B256::ZERO, B256::ZERO);
    let txs: Vec<Bytes> = vec![];

    let outcome = executor.execute(&state, &context, &txs).expect("execution should succeed");

    assert!(outcome.changes.is_empty());
    assert!(outcome.receipts.is_empty());
    assert_eq!(outcome.gas_used, 0);
}

#[rstest]
#[case(1)]
#[case(137)]
#[case(42161)]
fn test_execute_empty_transactions_different_chains(#[case] chain_id: u64) {
    let executor = RevmExecutor::new(chain_id);
    let state = MockStateDb::new();
    let context = BlockContext::new(Header::default(), B256::ZERO, B256::ZERO);
    let txs: Vec<Bytes> = vec![];

    let outcome = executor.execute(&state, &context, &txs).expect("execution should succeed");

    assert!(outcome.changes.is_empty());
    assert!(outcome.receipts.is_empty());
    assert_eq!(outcome.gas_used, 0);
}

// ----------------------------------------------------------------------------
// Tests for validate_header
// ----------------------------------------------------------------------------

#[test]
fn test_validate_header_succeeds_with_valid_gas_limit() {
    let executor = RevmExecutor::new(1);
    let header = Header { gas_limit: 30_000_000, ..Default::default() };

    let result = <RevmExecutor as BlockExecutor<MockStateDb>>::validate_header(&executor, &header);

    assert!(result.is_ok());
}

#[test]
fn test_validate_header_fails_with_gas_limit_below_minimum() {
    let executor = RevmExecutor::new(1);
    let header = Header { gas_limit: 1000, ..Default::default() };

    let result = <RevmExecutor as BlockExecutor<MockStateDb>>::validate_header(&executor, &header);

    assert!(result.is_err());
}

#[rstest]
#[case(0, 30_000_000)]
#[case(1, 30_000_000)]
#[case(1_000_000, 30_000_000)]
#[case(u64::MAX, 30_000_000)]
fn test_validate_header_succeeds_with_various_block_numbers(#[case] number: u64, #[case] gas_limit: u64) {
    let executor = RevmExecutor::new(1);
    let header = Header { number, gas_limit, ..Default::default() };

    let result = <RevmExecutor as BlockExecutor<MockStateDb>>::validate_header(&executor, &header);

    assert!(result.is_ok());
}

// ----------------------------------------------------------------------------
// Tests for BlockContext creation and field access
// ----------------------------------------------------------------------------

#[test]
fn test_block_context_creation_with_defaults() {
    let header = Header::default();
    let parent_hash = B256::repeat_byte(1);
    let prevrandao = B256::ZERO;

    let context = BlockContext::new(header.clone(), parent_hash, prevrandao);

    assert_eq!(context.prevrandao, B256::ZERO);
    assert_eq!(context.parent_hash, parent_hash);
    assert_eq!(context.header.number, header.number);
}

#[test]
fn test_block_context_creation_with_custom_prevrandao() {
    let header = Header::default();
    let prevrandao = B256::from([0xAB; 32]);

    let context = BlockContext::new(header, B256::ZERO, prevrandao);

    assert_eq!(context.prevrandao, B256::from([0xAB; 32]));
}

#[rstest]
#[case(0, 0)]
#[case(1, 1000)]
#[case(100, 21000)]
#[case(u64::MAX, u64::MAX)]
fn test_block_context_with_various_header_values(#[case] number: u64, #[case] gas_limit: u64) {
    let header = Header { number, gas_limit, ..Default::default() };
    let prevrandao = B256::from([number as u8; 32]);

    let context = BlockContext::new(header, B256::ZERO, prevrandao);

    assert_eq!(context.header.number, number);
    assert_eq!(context.header.gas_limit, gas_limit);
    assert_eq!(context.prevrandao, prevrandao);
}

// ----------------------------------------------------------------------------
// Tests for MockStateDb (validates our test infrastructure)
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_mock_state_db_account_not_found() {
    let state = MockStateDb::new();
    let address = Address::from([0x01; 20]);

    let result = state.nonce(&address).await;

    assert!(matches!(result, Err(StateDbError::AccountNotFound(_))));
}

#[tokio::test]
async fn test_mock_state_db_insert_and_read_account() {
    let state = MockStateDb::new();
    let address = Address::from([0x01; 20]);
    let account = MockAccount {
        nonce: 5,
        balance: U256::from(1000),
        code_hash: B256::ZERO,
        storage: HashMap::new(),
    };

    state.insert_account(address, account);

    assert_eq!(state.nonce(&address).await.unwrap(), 5);
    assert_eq!(state.balance(&address).await.unwrap(), U256::from(1000));
}

#[tokio::test]
async fn test_mock_state_db_storage_returns_zero_for_missing_slot() {
    let state = MockStateDb::new();
    let address = Address::from([0x01; 20]);
    let account = MockAccount::default();
    state.insert_account(address, account);

    let slot = U256::from(42);
    let value = state.storage(&address, &slot).await.unwrap();

    assert_eq!(value, U256::ZERO);
}

#[tokio::test]
async fn test_mock_state_db_storage_returns_zero_for_missing_account() {
    let state = MockStateDb::new();
    let address = Address::from([0x01; 20]);
    let slot = U256::from(42);

    let value = state.storage(&address, &slot).await.unwrap();

    assert_eq!(value, U256::ZERO);
}

#[tokio::test]
async fn test_mock_state_db_storage_returns_value_for_existing_slot() {
    let state = MockStateDb::new();
    let address = Address::from([0x01; 20]);
    let mut storage = HashMap::new();
    storage.insert(U256::from(42), U256::from(999));
    let account = MockAccount { storage, ..Default::default() };
    state.insert_account(address, account);

    let value = state.storage(&address, &U256::from(42)).await.unwrap();

    assert_eq!(value, U256::from(999));
}

#[tokio::test]
async fn test_mock_state_db_commit_stores_changes() {
    let state = MockStateDb::new();
    let address = Address::from([0x01; 20]);

    let mut changes = ChangeSet::new();
    changes.insert(
        address,
        AccountUpdate {
            created: true,
            selfdestructed: false,
            nonce: 10,
            balance: U256::from(5000),
            code_hash: B256::ZERO,
            code: None,
            storage: std::collections::BTreeMap::new(),
        },
    );

    let root = state.commit(changes).await.unwrap();

    assert_ne!(root, B256::ZERO);
    assert_eq!(state.nonce(&address).await.unwrap(), 10);
    assert_eq!(state.balance(&address).await.unwrap(), U256::from(5000));
}

#[tokio::test]
async fn test_mock_state_db_commit_handles_selfdestruct() {
    let state = MockStateDb::new();
    let address = Address::from([0x01; 20]);

    // First create the account.
    state.insert_account(
        address,
        MockAccount { nonce: 5, balance: U256::from(1000), ..Default::default() },
    );

    // Then selfdestruct it.
    let mut changes = ChangeSet::new();
    changes.insert(
        address,
        AccountUpdate {
            created: false,
            selfdestructed: true,
            nonce: 0,
            balance: U256::ZERO,
            code_hash: B256::ZERO,
            code: None,
            storage: std::collections::BTreeMap::new(),
        },
    );

    state.commit(changes).await.unwrap();

    assert!(matches!(state.nonce(&address).await, Err(StateDbError::AccountNotFound(_))));
}

#[tokio::test]
async fn test_mock_state_db_commit_stores_code() {
    let state = MockStateDb::new();
    let address = Address::from([0x01; 20]);
    let code_hash = B256::from([0xCC; 32]);
    let code = vec![0x60, 0x00, 0x60, 0x00];

    let mut changes = ChangeSet::new();
    changes.insert(
        address,
        AccountUpdate {
            created: true,
            selfdestructed: false,
            nonce: 0,
            balance: U256::ZERO,
            code_hash,
            code: Some(code.clone()),
            storage: std::collections::BTreeMap::new(),
        },
    );

    state.commit(changes).await.unwrap();

    assert_eq!(state.code(&code_hash).await.unwrap(), Bytes::from(code));
}

#[tokio::test]
async fn test_mock_state_db_code_not_found() {
    let state = MockStateDb::new();
    let code_hash = B256::from([0xCC; 32]);

    let result = state.code(&code_hash).await;

    assert!(matches!(result, Err(StateDbError::CodeNotFound(_))));
}

#[tokio::test]
async fn test_mock_state_db_insert_code() {
    let state = MockStateDb::new();
    let code_hash = B256::from([0xCC; 32]);
    let code = Bytes::from(vec![0x60, 0x00]);

    state.insert_code(code_hash, code.clone());

    assert_eq!(state.code(&code_hash).await.unwrap(), code);
}

#[tokio::test]
async fn test_mock_state_db_state_root() {
    let state = MockStateDb::new();

    let root = state.state_root().await.unwrap();

    assert_eq!(root, B256::ZERO);
}

#[test]
fn test_mock_state_db_merge_changes() {
    let state = MockStateDb::new();
    let address = Address::from([0x01; 20]);

    let mut older = ChangeSet::new();
    older.insert(
        address,
        AccountUpdate {
            created: true,
            selfdestructed: false,
            nonce: 1,
            balance: U256::from(100),
            code_hash: B256::ZERO,
            code: None,
            storage: std::collections::BTreeMap::new(),
        },
    );

    let mut newer = ChangeSet::new();
    newer.insert(
        address,
        AccountUpdate {
            created: false,
            selfdestructed: false,
            nonce: 5,
            balance: U256::from(500),
            code_hash: B256::ZERO,
            code: None,
            storage: std::collections::BTreeMap::new(),
        },
    );

    let merged = state.merge_changes(older, newer);

    let update = merged.accounts.get(&address).unwrap();
    assert_eq!(update.nonce, 5);
    assert_eq!(update.balance, U256::from(500));
}

// ----------------------------------------------------------------------------
// Tests for exists() default implementation
// ----------------------------------------------------------------------------

#[tokio::test]
async fn test_mock_state_db_exists_returns_false_for_missing_account() {
    let state = MockStateDb::new();
    let address = Address::from([0x01; 20]);

    assert!(!state.exists(&address).await.unwrap());
}

#[tokio::test]
async fn test_mock_state_db_exists_returns_true_for_account_with_nonce() {
    let state = MockStateDb::new();
    let address = Address::from([0x01; 20]);
    state.insert_account(address, MockAccount { nonce: 1, ..Default::default() });

    assert!(state.exists(&address).await.unwrap());
}

#[tokio::test]
async fn test_mock_state_db_exists_returns_true_for_account_with_balance() {
    let state = MockStateDb::new();
    let address = Address::from([0x01; 20]);
    state.insert_account(
        address,
        MockAccount { nonce: 0, balance: U256::from(1), ..Default::default() },
    );

    assert!(state.exists(&address).await.unwrap());
}

#[tokio::test]
async fn test_mock_state_db_exists_returns_false_for_empty_account() {
    let state = MockStateDb::new();
    let address = Address::from([0x01; 20]);
    state.insert_account(address, MockAccount::default());

    assert!(!state.exists(&address).await.unwrap());
}

// ----------------------------------------------------------------------------
// Tests for executor with populated state
// ----------------------------------------------------------------------------

#[test]
fn test_execute_with_populated_state() {
    let executor = RevmExecutor::new(1);
    let state = MockStateDb::new();

    // Populate some accounts.
    let alice = Address::from([0x01; 20]);
    let bob = Address::from([0x02; 20]);
    state.insert_account(
        alice,
        MockAccount { nonce: 1, balance: U256::from(1000), ..Default::default() },
    );
    state.insert_account(
        bob,
        MockAccount { nonce: 0, balance: U256::from(500), ..Default::default() },
    );

    let context = BlockContext::new(Header::default(), B256::ZERO, B256::ZERO);
    let txs: Vec<Bytes> = vec![];

    let outcome = executor.execute(&state, &context, &txs).expect("execution should succeed");

    // Empty transactions still produce empty outcome.
    assert!(outcome.changes.is_empty());
    assert!(outcome.receipts.is_empty());
    assert_eq!(outcome.gas_used, 0);
}
