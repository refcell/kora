//! State change tracking with merge capability.

use std::collections::BTreeMap;

use alloy_primitives::{Address, B256, U256};

/// Accumulated state changes that can be merged across blocks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChangeSet {
    /// Account changes keyed by address.
    pub accounts: BTreeMap<Address, AccountUpdate>,
}

impl ChangeSet {
    /// Create an empty change set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there are no changes.
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    /// Number of accounts with changes.
    pub fn len(&self) -> usize {
        self.accounts.len()
    }

    /// Merge another change set into this one.
    pub fn merge(&mut self, other: Self) {
        for (address, update) in other.accounts {
            if let Some(existing) = self.accounts.get_mut(&address) {
                existing.merge(update);
            } else {
                self.accounts.insert(address, update);
            }
        }
    }

    /// Insert or update an account.
    pub fn insert(&mut self, address: Address, update: AccountUpdate) {
        if let Some(existing) = self.accounts.get_mut(&address) {
            existing.merge(update);
        } else {
            self.accounts.insert(address, update);
        }
    }
}

/// State changes for a single account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountUpdate {
    /// Whether account was created in this change.
    pub created: bool,
    /// Whether account was selfdestructed.
    pub selfdestructed: bool,
    /// Current nonce.
    pub nonce: u64,
    /// Current balance.
    pub balance: U256,
    /// Code hash.
    pub code_hash: B256,
    /// New code bytes (if code was deployed).
    pub code: Option<Vec<u8>>,
    /// Storage slot changes.
    pub storage: BTreeMap<U256, U256>,
}

impl AccountUpdate {
    /// Merge another update into this one.
    pub fn merge(&mut self, other: Self) {
        let Self { created, selfdestructed, nonce, balance, code_hash, code, storage } = other;

        if created {
            self.storage.clear();
            self.created = true;
        }

        if selfdestructed {
            self.storage.clear();
        }

        self.selfdestructed = selfdestructed;
        self.nonce = nonce;
        self.balance = balance;

        if self.code_hash != code_hash || code.is_some() {
            self.code = code;
        }
        self.code_hash = code_hash;

        if !selfdestructed {
            for (slot, value) in storage {
                self.storage.insert(slot, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_overwrites_nonce_and_balance() {
        let mut cs1 = ChangeSet::new();
        cs1.accounts.insert(
            Address::ZERO,
            AccountUpdate {
                created: false,
                selfdestructed: false,
                nonce: 1,
                balance: U256::from(100),
                code_hash: B256::ZERO,
                code: None,
                storage: BTreeMap::new(),
            },
        );

        let mut cs2 = ChangeSet::new();
        cs2.accounts.insert(
            Address::ZERO,
            AccountUpdate {
                created: false,
                selfdestructed: false,
                nonce: 5,
                balance: U256::from(500),
                code_hash: B256::ZERO,
                code: None,
                storage: BTreeMap::new(),
            },
        );

        cs1.merge(cs2);
        let update = cs1.accounts.get(&Address::ZERO).unwrap();
        assert_eq!(update.nonce, 5);
        assert_eq!(update.balance, U256::from(500));
    }

    #[test]
    fn selfdestruct_clears_storage() {
        let mut update = AccountUpdate {
            created: false,
            selfdestructed: false,
            nonce: 1,
            balance: U256::from(100),
            code_hash: B256::ZERO,
            code: None,
            storage: BTreeMap::from([(U256::from(1), U256::from(999))]),
        };

        update.merge(AccountUpdate {
            created: false,
            selfdestructed: true,
            nonce: 0,
            balance: U256::ZERO,
            code_hash: B256::ZERO,
            code: None,
            storage: BTreeMap::new(),
        });

        assert!(update.selfdestructed);
        assert!(update.storage.is_empty());
    }
}
