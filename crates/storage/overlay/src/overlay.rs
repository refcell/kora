use std::sync::Arc;

use alloy_primitives::{Address, B256, Bytes, U256};
use kora_qmdb::ChangeSet;
use kora_traits::{StateDb, StateDbError, StateDbRead, StateDbWrite};

/// State overlay that layers pending changes on top of a base state database.
#[derive(Clone, Debug)]
pub struct OverlayState<S> {
    base: S,
    changes: Arc<ChangeSet>,
}

impl<S> OverlayState<S> {
    /// Create a new overlay from a base state and a change set.
    #[must_use]
    pub fn new(base: S, changes: ChangeSet) -> Self {
        Self { base, changes: Arc::new(changes) }
    }

    /// Merge the current overlay changes with a newer change set.
    pub fn merge_changes(&self, newer: ChangeSet) -> ChangeSet {
        let mut merged = (*self.changes).clone();
        merged.merge(newer);
        merged
    }
}

impl<S: Clone> OverlayState<S> {
    /// Return a clone of the base state handle.
    pub fn base(&self) -> S {
        self.base.clone()
    }
}

impl<S: StateDbRead> StateDbRead for OverlayState<S> {
    fn nonce(
        &self,
        address: &Address,
    ) -> impl std::future::Future<Output = Result<u64, StateDbError>> + Send {
        let address = *address;
        let base = self.base.clone();
        let changes = Arc::clone(&self.changes);
        async move {
            if let Some(update) = changes.accounts.get(&address) {
                return Ok(update.nonce);
            }
            base.nonce(&address).await
        }
    }

    fn balance(
        &self,
        address: &Address,
    ) -> impl std::future::Future<Output = Result<U256, StateDbError>> + Send {
        let address = *address;
        let base = self.base.clone();
        let changes = Arc::clone(&self.changes);
        async move {
            if let Some(update) = changes.accounts.get(&address) {
                return Ok(update.balance);
            }
            base.balance(&address).await
        }
    }

    fn code_hash(
        &self,
        address: &Address,
    ) -> impl std::future::Future<Output = Result<B256, StateDbError>> + Send {
        let address = *address;
        let base = self.base.clone();
        let changes = Arc::clone(&self.changes);
        async move {
            if let Some(update) = changes.accounts.get(&address) {
                return Ok(update.code_hash);
            }
            base.code_hash(&address).await
        }
    }

    fn code(
        &self,
        code_hash: &B256,
    ) -> impl std::future::Future<Output = Result<Bytes, StateDbError>> + Send {
        let code_hash = *code_hash;
        let base = self.base.clone();
        let changes = Arc::clone(&self.changes);
        async move {
            for update in changes.accounts.values() {
                if update.code_hash == code_hash
                    && let Some(code) = &update.code
                {
                    return Ok(Bytes::from(code.clone()));
                }
            }
            base.code(&code_hash).await
        }
    }

    fn storage(
        &self,
        address: &Address,
        slot: &U256,
    ) -> impl std::future::Future<Output = Result<U256, StateDbError>> + Send {
        let address = *address;
        let slot = *slot;
        let base = self.base.clone();
        let changes = Arc::clone(&self.changes);
        async move {
            if let Some(update) = changes.accounts.get(&address) {
                if update.selfdestructed {
                    return Ok(U256::ZERO);
                }
                if let Some(value) = update.storage.get(&slot) {
                    return Ok(*value);
                }
                if update.created {
                    return Ok(U256::ZERO);
                }
            }
            base.storage(&address, &slot).await
        }
    }
}

impl<S: StateDbWrite> StateDbWrite for OverlayState<S> {
    fn commit(
        &self,
        changes: ChangeSet,
    ) -> impl std::future::Future<Output = Result<B256, StateDbError>> + Send {
        let base = self.base.clone();
        let overlay = Arc::clone(&self.changes);
        async move {
            let mut merged = (*overlay).clone();
            merged.merge(changes);
            base.commit(merged).await
        }
    }

    fn compute_root(
        &self,
        changes: &ChangeSet,
    ) -> impl std::future::Future<Output = Result<B256, StateDbError>> + Send {
        let base = self.base.clone();
        let overlay = Arc::clone(&self.changes);
        let changes = changes.clone();
        async move {
            let mut merged = (*overlay).clone();
            merged.merge(changes);
            base.compute_root(&merged).await
        }
    }

    fn merge_changes(&self, older: ChangeSet, newer: ChangeSet) -> ChangeSet {
        self.base.merge_changes(older, newer)
    }
}

impl<S: StateDb> StateDb for OverlayState<S> {
    fn state_root(&self) -> impl std::future::Future<Output = Result<B256, StateDbError>> + Send {
        let base = self.base.clone();
        async move { base.state_root().await }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use kora_qmdb::AccountUpdate;

    use super::*;

    #[derive(Clone, Debug)]
    struct MockStateDb {
        accounts: BTreeMap<Address, AccountUpdate>,
    }

    impl MockStateDb {
        fn new() -> Self {
            Self { accounts: BTreeMap::new() }
        }

        fn with_account(mut self, address: Address, update: AccountUpdate) -> Self {
            self.accounts.insert(address, update);
            self
        }
    }

    impl StateDbRead for MockStateDb {
        fn nonce(
            &self,
            address: &Address,
        ) -> impl std::future::Future<Output = Result<u64, StateDbError>> + Send {
            let nonce = self.accounts.get(address).map(|a| a.nonce).unwrap_or(0);
            async move { Ok(nonce) }
        }

        fn balance(
            &self,
            address: &Address,
        ) -> impl std::future::Future<Output = Result<U256, StateDbError>> + Send {
            let balance = self.accounts.get(address).map(|a| a.balance).unwrap_or(U256::ZERO);
            async move { Ok(balance) }
        }

        fn code_hash(
            &self,
            address: &Address,
        ) -> impl std::future::Future<Output = Result<B256, StateDbError>> + Send {
            let hash = self.accounts.get(address).map(|a| a.code_hash).unwrap_or(B256::ZERO);
            async move { Ok(hash) }
        }

        fn code(
            &self,
            code_hash: &B256,
        ) -> impl std::future::Future<Output = Result<Bytes, StateDbError>> + Send {
            let code_hash = *code_hash;
            let code = self
                .accounts
                .values()
                .find(|a| a.code_hash == code_hash)
                .and_then(|a| a.code.clone())
                .map(Bytes::from)
                .unwrap_or_default();
            async move { Ok(code) }
        }

        fn storage(
            &self,
            address: &Address,
            slot: &U256,
        ) -> impl std::future::Future<Output = Result<U256, StateDbError>> + Send {
            let value = self
                .accounts
                .get(address)
                .and_then(|a| a.storage.get(slot).copied())
                .unwrap_or(U256::ZERO);
            async move { Ok(value) }
        }
    }

    fn test_account(nonce: u64, balance: u64) -> AccountUpdate {
        AccountUpdate {
            created: false,
            selfdestructed: false,
            nonce,
            balance: U256::from(balance),
            code_hash: B256::ZERO,
            code: None,
            storage: BTreeMap::new(),
        }
    }

    fn test_account_with_storage(
        nonce: u64,
        balance: u64,
        slot: U256,
        value: U256,
    ) -> AccountUpdate {
        let mut storage = BTreeMap::new();
        storage.insert(slot, value);
        AccountUpdate {
            created: false,
            selfdestructed: false,
            nonce,
            balance: U256::from(balance),
            code_hash: B256::ZERO,
            code: None,
            storage,
        }
    }

    #[tokio::test]
    async fn test_overlay_returns_base_when_no_changes() {
        let addr = Address::repeat_byte(0x01);
        let base = MockStateDb::new().with_account(addr, test_account(5, 1000));
        let overlay = OverlayState::new(base, ChangeSet::new());

        assert_eq!(overlay.nonce(&addr).await.unwrap(), 5);
        assert_eq!(overlay.balance(&addr).await.unwrap(), U256::from(1000));
    }

    #[tokio::test]
    async fn test_overlay_returns_changes_over_base() {
        let addr = Address::repeat_byte(0x01);
        let base = MockStateDb::new().with_account(addr, test_account(5, 1000));

        let mut changes = ChangeSet::new();
        changes.accounts.insert(addr, test_account(10, 2000));

        let overlay = OverlayState::new(base, changes);

        assert_eq!(overlay.nonce(&addr).await.unwrap(), 10);
        assert_eq!(overlay.balance(&addr).await.unwrap(), U256::from(2000));
    }

    #[tokio::test]
    async fn test_overlay_storage_from_changes() {
        let addr = Address::repeat_byte(0x02);
        let slot = U256::from(42);
        let value = U256::from(999);

        let base = MockStateDb::new();
        let mut changes = ChangeSet::new();
        changes.accounts.insert(addr, test_account_with_storage(1, 100, slot, value));

        let overlay = OverlayState::new(base, changes);

        assert_eq!(overlay.storage(&addr, &slot).await.unwrap(), value);
    }

    #[tokio::test]
    async fn test_overlay_storage_falls_back_to_base() {
        let addr = Address::repeat_byte(0x03);
        let slot = U256::from(10);
        let value = U256::from(555);

        let base =
            MockStateDb::new().with_account(addr, test_account_with_storage(1, 100, slot, value));
        let overlay = OverlayState::new(base, ChangeSet::new());

        assert_eq!(overlay.storage(&addr, &slot).await.unwrap(), value);
    }

    #[tokio::test]
    async fn test_overlay_selfdestructed_returns_zero_storage() {
        let addr = Address::repeat_byte(0x04);
        let slot = U256::from(1);

        let base = MockStateDb::new()
            .with_account(addr, test_account_with_storage(1, 100, slot, U256::from(777)));

        let mut changes = ChangeSet::new();
        changes.accounts.insert(
            addr,
            AccountUpdate {
                created: false,
                selfdestructed: true,
                nonce: 0,
                balance: U256::ZERO,
                code_hash: B256::ZERO,
                code: None,
                storage: BTreeMap::new(),
            },
        );

        let overlay = OverlayState::new(base, changes);

        assert_eq!(overlay.storage(&addr, &slot).await.unwrap(), U256::ZERO);
    }

    #[tokio::test]
    async fn test_overlay_created_account_returns_zero_for_missing_storage() {
        let addr = Address::repeat_byte(0x05);
        let slot = U256::from(99);

        let base = MockStateDb::new()
            .with_account(addr, test_account_with_storage(1, 100, slot, U256::from(123)));

        let mut changes = ChangeSet::new();
        changes.accounts.insert(
            addr,
            AccountUpdate {
                created: true,
                selfdestructed: false,
                nonce: 0,
                balance: U256::ZERO,
                code_hash: B256::ZERO,
                code: None,
                storage: BTreeMap::new(),
            },
        );

        let overlay = OverlayState::new(base, changes);

        assert_eq!(overlay.storage(&addr, &slot).await.unwrap(), U256::ZERO);
    }

    #[test]
    fn test_merge_changes_combines_changesets() {
        let addr1 = Address::repeat_byte(0x01);
        let addr2 = Address::repeat_byte(0x02);

        let mut cs1 = ChangeSet::new();
        cs1.accounts.insert(addr1, test_account(1, 100));

        let mut cs2 = ChangeSet::new();
        cs2.accounts.insert(addr2, test_account(2, 200));

        let base = MockStateDb::new();
        let overlay = OverlayState::new(base, cs1);
        let merged = overlay.merge_changes(cs2);

        assert!(merged.accounts.contains_key(&addr1));
        assert!(merged.accounts.contains_key(&addr2));
    }

    #[test]
    fn test_base_accessor() {
        let base = MockStateDb::new();
        let overlay = OverlayState::new(base, ChangeSet::new());
        let _ = overlay.base();
    }

    #[tokio::test]
    async fn test_overlay_code_hash_from_changes() {
        let addr = Address::repeat_byte(0x06);
        let code_hash = B256::repeat_byte(0xAB);

        let base = MockStateDb::new();
        let mut changes = ChangeSet::new();
        changes.accounts.insert(
            addr,
            AccountUpdate {
                created: true,
                selfdestructed: false,
                nonce: 1,
                balance: U256::from(500),
                code_hash,
                code: Some(vec![0x60, 0x00]),
                storage: BTreeMap::new(),
            },
        );

        let overlay = OverlayState::new(base, changes);

        assert_eq!(overlay.code_hash(&addr).await.unwrap(), code_hash);
    }

    #[tokio::test]
    async fn test_overlay_code_hash_falls_back_to_base() {
        let addr = Address::repeat_byte(0x07);
        let code_hash = B256::repeat_byte(0xCD);

        let base = MockStateDb::new().with_account(
            addr,
            AccountUpdate {
                created: false,
                selfdestructed: false,
                nonce: 0,
                balance: U256::ZERO,
                code_hash,
                code: None,
                storage: BTreeMap::new(),
            },
        );
        let overlay = OverlayState::new(base, ChangeSet::new());

        assert_eq!(overlay.code_hash(&addr).await.unwrap(), code_hash);
    }

    #[tokio::test]
    async fn test_overlay_code_from_changes() {
        let addr = Address::repeat_byte(0x08);
        let code_hash = B256::repeat_byte(0xEF);
        let code_bytes = vec![0x60, 0x00, 0x60, 0x00];

        let base = MockStateDb::new();
        let mut changes = ChangeSet::new();
        changes.accounts.insert(
            addr,
            AccountUpdate {
                created: true,
                selfdestructed: false,
                nonce: 1,
                balance: U256::from(100),
                code_hash,
                code: Some(code_bytes.clone()),
                storage: BTreeMap::new(),
            },
        );

        let overlay = OverlayState::new(base, changes);

        assert_eq!(overlay.code(&code_hash).await.unwrap(), Bytes::from(code_bytes));
    }

    #[tokio::test]
    async fn test_overlay_code_falls_back_to_base() {
        let addr = Address::repeat_byte(0x09);
        let code_hash = B256::repeat_byte(0x12);
        let code_bytes = vec![0x61, 0x02, 0x03];

        let base = MockStateDb::new().with_account(
            addr,
            AccountUpdate {
                created: false,
                selfdestructed: false,
                nonce: 0,
                balance: U256::ZERO,
                code_hash,
                code: Some(code_bytes.clone()),
                storage: BTreeMap::new(),
            },
        );
        let overlay = OverlayState::new(base, ChangeSet::new());

        assert_eq!(overlay.code(&code_hash).await.unwrap(), Bytes::from(code_bytes));
    }

    #[tokio::test]
    async fn test_overlay_code_returns_empty_for_unknown_hash() {
        let base = MockStateDb::new();
        let overlay = OverlayState::new(base, ChangeSet::new());
        let unknown_hash = B256::repeat_byte(0xFF);

        assert_eq!(overlay.code(&unknown_hash).await.unwrap(), Bytes::new());
    }
}
