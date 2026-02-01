//! Commonware QMDB type aliases and codecs.

use alloy_primitives::U256;
use bytes::{Buf, BufMut};
use commonware_codec::{EncodeSize, Error as CodecError, Read, Write};
use commonware_cryptography::sha256::Sha256 as QmdbHasher;
use commonware_runtime::tokio;
use commonware_storage::{
    qmdb::{NonDurable, Unmerkleized, any},
    translator::EightCap,
};
use commonware_utils::sequence::FixedBytes;
use kora_qmdb::AccountEncoding;

use crate::BackendError;

pub(crate) type Context = tokio::Context;
pub(crate) type AccountKey = FixedBytes<20>;
pub(crate) type StorageKey = FixedBytes<60>;
pub(crate) type CodeKey = FixedBytes<32>;

#[derive(Clone, Debug)]
pub(crate) struct AccountValue(pub [u8; AccountEncoding::SIZE]);

impl Write for AccountValue {
    fn write(&self, buf: &mut impl BufMut) {
        buf.put_slice(&self.0);
    }
}

impl EncodeSize for AccountValue {
    fn encode_size(&self) -> usize {
        AccountEncoding::SIZE
    }
}

impl Read for AccountValue {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &Self::Cfg) -> Result<Self, CodecError> {
        if buf.remaining() < AccountEncoding::SIZE {
            return Err(CodecError::EndOfBuffer);
        }
        let mut out = [0u8; AccountEncoding::SIZE];
        buf.copy_to_slice(&mut out);
        Ok(Self(out))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct StorageValue(pub U256);

impl Write for StorageValue {
    fn write(&self, buf: &mut impl BufMut) {
        buf.put_slice(&self.0.to_be_bytes::<32>());
    }
}

impl EncodeSize for StorageValue {
    fn encode_size(&self) -> usize {
        32
    }
}

impl Read for StorageValue {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &Self::Cfg) -> Result<Self, CodecError> {
        if buf.remaining() < 32 {
            return Err(CodecError::EndOfBuffer);
        }
        let mut out = [0u8; 32];
        buf.copy_to_slice(&mut out);
        Ok(Self(U256::from_be_bytes(out)))
    }
}

pub(crate) type AccountDb =
    any::unordered::variable::Db<Context, AccountKey, AccountValue, QmdbHasher, EightCap>;
pub(crate) type StorageDb =
    any::unordered::variable::Db<Context, StorageKey, StorageValue, QmdbHasher, EightCap>;
pub(crate) type CodeDb =
    any::unordered::variable::Db<Context, CodeKey, Vec<u8>, QmdbHasher, EightCap>;

pub(crate) type AccountDbDirty = any::unordered::variable::Db<
    Context,
    AccountKey,
    AccountValue,
    QmdbHasher,
    EightCap,
    Unmerkleized,
    NonDurable,
>;
pub(crate) type StorageDbDirty = any::unordered::variable::Db<
    Context,
    StorageKey,
    StorageValue,
    QmdbHasher,
    EightCap,
    Unmerkleized,
    NonDurable,
>;
pub(crate) type CodeDbDirty = any::unordered::variable::Db<
    Context,
    CodeKey,
    Vec<u8>,
    QmdbHasher,
    EightCap,
    Unmerkleized,
    NonDurable,
>;

pub(crate) struct StoreSlot<T>(Option<T>);

impl<T> StoreSlot<T> {
    pub(crate) const fn new(inner: T) -> Self {
        Self(Some(inner))
    }

    pub(crate) fn get(&self) -> Result<&T, BackendError> {
        self.0.as_ref().ok_or(BackendError::NotInitialized)
    }

    pub(crate) fn take(&mut self) -> Result<T, BackendError> {
        self.0.take().ok_or(BackendError::NotInitialized)
    }

    pub(crate) fn restore(&mut self, inner: T) {
        self.0 = Some(inner);
    }

    pub(crate) fn into_inner(self) -> Result<T, BackendError> {
        self.0.ok_or(BackendError::NotInitialized)
    }
}

#[cfg(test)]
mod tests {
    use commonware_codec::{DecodeExt, Encode};

    use super::*;

    #[test]
    fn test_account_value_roundtrip() {
        let mut data = [0u8; AccountEncoding::SIZE];
        data[0] = 0x42;
        data[79] = 0xFF;
        let value = AccountValue(data);

        let encoded = value.encode();
        let decoded = AccountValue::decode(encoded).unwrap();
        assert_eq!(decoded.0, data);
    }

    #[test]
    fn test_account_value_encode_size() {
        let value = AccountValue([0u8; AccountEncoding::SIZE]);
        assert_eq!(value.encode_size(), AccountEncoding::SIZE);
    }

    #[test]
    fn test_storage_value_roundtrip() {
        let value = StorageValue(U256::from(12345678u64));
        let encoded = value.encode();
        let decoded = StorageValue::decode(encoded).unwrap();
        assert_eq!(decoded.0, value.0);
    }

    #[test]
    fn test_storage_value_max() {
        let value = StorageValue(U256::MAX);
        let encoded = value.encode();
        let decoded = StorageValue::decode(encoded).unwrap();
        assert_eq!(decoded.0, U256::MAX);
    }

    #[test]
    fn test_storage_value_encode_size() {
        let value = StorageValue(U256::ZERO);
        assert_eq!(value.encode_size(), 32);
    }

    #[test]
    fn test_store_slot_get_succeeds() {
        let slot = StoreSlot::new(42);
        assert_eq!(*slot.get().unwrap(), 42);
    }

    #[test]
    fn test_store_slot_take_removes_value() {
        let mut slot = StoreSlot::new(42);
        assert_eq!(slot.take().unwrap(), 42);
        assert!(slot.get().is_err());
    }

    #[test]
    fn test_store_slot_take_twice_fails() {
        let mut slot = StoreSlot::new(42);
        slot.take().unwrap();
        assert!(slot.take().is_err());
    }

    #[test]
    fn test_store_slot_restore_after_take() {
        let mut slot = StoreSlot::new(42);
        slot.take().unwrap();
        slot.restore(100);
        assert_eq!(*slot.get().unwrap(), 100);
    }

    #[test]
    fn test_store_slot_into_inner_succeeds() {
        let slot = StoreSlot::new(42);
        assert_eq!(slot.into_inner().unwrap(), 42);
    }

    #[test]
    fn test_store_slot_into_inner_after_take_fails() {
        let mut slot = StoreSlot::new(42);
        slot.take().unwrap();
        assert!(slot.into_inner().is_err());
    }
}
