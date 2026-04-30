//! State root computation.

use alloy_primitives::{B256, keccak256};

use crate::ChangeSet;

const KORA_ROOT_NAMESPACE: &[u8] = b"_KORA_QMDB_ROOT";
const KORA_TRANSITION_ROOT_NAMESPACE: &[u8] = b"_KORA_STATE_TRANSITION_ROOT";

/// State root computation utility.
#[derive(Debug, Clone, Copy)]
pub struct StateRoot;

impl StateRoot {
    /// Compute state root from three partition roots.
    pub fn compute(accounts_root: B256, storage_root: B256, code_root: B256) -> B256 {
        let mut buf = Vec::with_capacity(KORA_ROOT_NAMESPACE.len() + 96);
        buf.extend_from_slice(KORA_ROOT_NAMESPACE);
        buf.extend_from_slice(accounts_root.as_slice());
        buf.extend_from_slice(storage_root.as_slice());
        buf.extend_from_slice(code_root.as_slice());
        keccak256(buf)
    }

    /// Compute a deterministic consensus root from a parent root and state transition.
    pub fn transition(parent_root: B256, changes: &ChangeSet) -> B256 {
        if changes.is_empty() {
            return parent_root;
        }

        let mut buf = Vec::new();
        buf.extend_from_slice(KORA_TRANSITION_ROOT_NAMESPACE);
        buf.extend_from_slice(parent_root.as_slice());
        buf.extend_from_slice(&(changes.accounts.len() as u64).to_be_bytes());

        for (address, update) in &changes.accounts {
            buf.extend_from_slice(address.as_slice());
            buf.push(u8::from(update.created));
            buf.push(u8::from(update.selfdestructed));
            buf.extend_from_slice(&update.nonce.to_be_bytes());
            buf.extend_from_slice(&update.balance.to_be_bytes::<32>());
            buf.extend_from_slice(update.code_hash.as_slice());

            match &update.code {
                Some(code) => {
                    buf.push(1);
                    buf.extend_from_slice(&(code.len() as u64).to_be_bytes());
                    buf.extend_from_slice(code);
                }
                None => buf.push(0),
            }

            buf.extend_from_slice(&(update.storage.len() as u64).to_be_bytes());
            for (slot, value) in &update.storage {
                buf.extend_from_slice(&slot.to_be_bytes::<32>());
                buf.extend_from_slice(&value.to_be_bytes::<32>());
            }
        }

        keccak256(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_root() {
        let a = B256::repeat_byte(0x11);
        let s = B256::repeat_byte(0x22);
        let c = B256::repeat_byte(0x33);

        let root1 = StateRoot::compute(a, s, c);
        let root2 = StateRoot::compute(a, s, c);
        assert_eq!(root1, root2);
    }

    #[test]
    fn different_inputs_different_root() {
        let root1 = StateRoot::compute(B256::ZERO, B256::ZERO, B256::ZERO);
        let root2 = StateRoot::compute(B256::repeat_byte(1), B256::ZERO, B256::ZERO);
        assert_ne!(root1, root2);
    }

    #[test]
    fn empty_transition_keeps_parent_root() {
        let parent = B256::repeat_byte(0x42);
        assert_eq!(StateRoot::transition(parent, &ChangeSet::new()), parent);
    }
}
