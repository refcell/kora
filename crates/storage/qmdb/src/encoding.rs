//! Key and value encoding for QMDB storage.

use alloy_primitives::{Address, B256, U256};

/// Storage key combining address, generation, and slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StorageKey {
    /// The account address.
    pub address: Address,
    /// Storage generation (incremented on account recreation).
    pub generation: u64,
    /// The storage slot.
    pub slot: U256,
}

impl StorageKey {
    /// Create a new storage key.
    #[must_use]
    pub const fn new(address: Address, generation: u64, slot: U256) -> Self {
        Self { address, generation, slot }
    }

    /// Encode to fixed-size bytes for QMDB.
    pub fn to_bytes(&self) -> [u8; 60] {
        let mut buf = [0u8; 60];
        buf[0..20].copy_from_slice(self.address.as_slice());
        buf[20..28].copy_from_slice(&self.generation.to_be_bytes());
        buf[28..60].copy_from_slice(&self.slot.to_be_bytes::<32>());
        buf
    }

    /// Decode from bytes.
    pub fn from_bytes(bytes: &[u8; 60]) -> Self {
        let address = Address::from_slice(&bytes[0..20]);
        let generation = u64::from_be_bytes(bytes[20..28].try_into().unwrap());
        let slot = U256::from_be_slice(&bytes[28..60]);
        Self { address, generation, slot }
    }
}

/// Account encoding utility.
///
/// Encodes account info as 80 bytes: nonce (8) + balance (32) + code_hash (32) + generation (8).
#[derive(Debug, Clone, Copy)]
pub struct AccountEncoding;

impl AccountEncoding {
    /// Encoded size in bytes.
    pub const SIZE: usize = 80;

    /// Encode account info for QMDB storage.
    pub fn encode(nonce: u64, balance: U256, code_hash: B256, generation: u64) -> [u8; 80] {
        let mut buf = [0u8; 80];
        buf[0..8].copy_from_slice(&nonce.to_be_bytes());
        buf[8..40].copy_from_slice(&balance.to_be_bytes::<32>());
        buf[40..72].copy_from_slice(code_hash.as_slice());
        buf[72..80].copy_from_slice(&generation.to_be_bytes());
        buf
    }

    /// Decode account info from QMDB storage.
    pub fn decode(bytes: &[u8]) -> Option<(u64, U256, B256, u64)> {
        if bytes.len() < Self::SIZE {
            return None;
        }
        let nonce = u64::from_be_bytes(bytes[0..8].try_into().ok()?);
        let balance = U256::from_be_slice(&bytes[8..40]);
        let code_hash = B256::from_slice(&bytes[40..72]);
        let generation = u64::from_be_bytes(bytes[72..80].try_into().ok()?);
        Some((nonce, balance, code_hash, generation))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(Address::repeat_byte(0xAB), 1, U256::from(42u64))]
    #[case(Address::ZERO, 0, U256::ZERO)]
    #[case(Address::repeat_byte(0xFF), u64::MAX, U256::MAX)]
    fn storage_key_roundtrip(
        #[case] address: Address,
        #[case] generation: u64,
        #[case] slot: U256,
    ) {
        let key = StorageKey::new(address, generation, slot);
        let bytes = key.to_bytes();
        let decoded = StorageKey::from_bytes(&bytes);
        assert_eq!(decoded, key);
    }

    #[rstest]
    #[case(123, U256::from(1_000_000u64), B256::repeat_byte(0xCD), 5)]
    #[case(0, U256::ZERO, B256::ZERO, 0)]
    #[case(u64::MAX, U256::MAX, B256::repeat_byte(0xFF), u64::MAX)]
    fn account_roundtrip(
        #[case] nonce: u64,
        #[case] balance: U256,
        #[case] code_hash: B256,
        #[case] generation: u64,
    ) {
        let encoded = AccountEncoding::encode(nonce, balance, code_hash, generation);
        let (n, b, c, g) = AccountEncoding::decode(&encoded).unwrap();
        assert_eq!((n, b, c, g), (nonce, balance, code_hash, generation));
    }
}
