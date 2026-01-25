//! Key and value encoding helpers for QMDB storage.

use kora_primitives::{Address, B256, U256};

/// Storage key combining address and slot for QMDB.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StorageKey {
    /// The account address.
    pub address: Address,
    /// The storage slot.
    pub slot: U256,
}

impl StorageKey {
    /// Create a new storage key.
    pub const fn new(address: Address, slot: U256) -> Self {
        Self { address, slot }
    }

    /// Encode to fixed-size bytes for QMDB.
    pub fn to_bytes(&self) -> [u8; 52] {
        let mut buf = [0u8; 52];
        buf[0..20].copy_from_slice(self.address.as_slice());
        buf[20..52].copy_from_slice(&self.slot.to_be_bytes::<32>());
        buf
    }
}

/// Encode account info for QMDB storage.
///
/// Format: nonce (8 bytes) | balance (32 bytes) | code_hash (32 bytes) = 72 bytes
pub fn encode_account(nonce: u64, balance: U256, code_hash: B256) -> [u8; 72] {
    let mut buf = [0u8; 72];
    buf[0..8].copy_from_slice(&nonce.to_be_bytes());
    buf[8..40].copy_from_slice(&balance.to_be_bytes::<32>());
    buf[40..72].copy_from_slice(code_hash.as_slice());
    buf
}

/// Decode account info from QMDB storage.
///
/// Returns (nonce, balance, code_hash) or None if bytes are too short.
pub fn decode_account(bytes: &[u8]) -> Option<(u64, U256, B256)> {
    if bytes.len() < 72 {
        return None;
    }
    let nonce = u64::from_be_bytes(bytes[0..8].try_into().ok()?);
    let balance = U256::from_be_slice(&bytes[8..40]);
    let code_hash = B256::from_slice(&bytes[40..72]);
    Some((nonce, balance, code_hash))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(Address::repeat_byte(0xAB), U256::from(42u64))]
    #[case(Address::ZERO, U256::ZERO)]
    #[case(Address::repeat_byte(0xFF), U256::MAX)]
    fn storage_key_roundtrip(#[case] address: Address, #[case] slot: U256) {
        let key = StorageKey::new(address, slot);
        let bytes = key.to_bytes();

        assert_eq!(&bytes[0..20], address.as_slice());
        assert_eq!(U256::from_be_slice(&bytes[20..52]), slot);
    }

    #[rstest]
    #[case(123u64, U256::from(1_000_000u64), B256::repeat_byte(0xCD))]
    #[case(0u64, U256::ZERO, B256::ZERO)]
    #[case(u64::MAX, U256::MAX, B256::repeat_byte(0xFF))]
    fn account_encoding_roundtrip(
        #[case] nonce: u64,
        #[case] balance: U256,
        #[case] code_hash: B256,
    ) {
        let encoded = encode_account(nonce, balance, code_hash);
        let (dec_nonce, dec_balance, dec_code_hash) = decode_account(&encoded).unwrap();

        assert_eq!(dec_nonce, nonce);
        assert_eq!(dec_balance, balance);
        assert_eq!(dec_code_hash, code_hash);
    }

    #[rstest]
    #[case(&[0u8; 0])]
    #[case(&[0u8; 71])]
    fn decode_account_too_short(#[case] bytes: &[u8]) {
        assert!(decode_account(bytes).is_none());
    }
}
