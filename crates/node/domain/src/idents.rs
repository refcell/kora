//! Identifiers

use alloy_evm::revm::primitives::B256;
use bytes::{Buf, BufMut};
use commonware_codec::{Error as CodecError, FixedSize, Read, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Block identifier (32 bytes).
pub struct BlockId(pub B256);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Transaction identifier (32 bytes).
pub struct TxId(pub B256);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// State commitment (32 bytes) computed from merkleized, non-durable QMDB partition roots.
pub struct StateRoot(pub B256);

/// Identifier encoding helpers.
#[derive(Debug)]
pub struct Idents;

impl Idents {
    /// Encode a `B256` into the buffer as raw bytes.
    pub fn write_b256(value: &B256, buf: &mut impl BufMut) {
        buf.put_slice(value.as_slice());
    }

    /// Decode a `B256` from the buffer, returning an error if insufficient bytes remain.
    pub fn read_b256(buf: &mut impl Buf) -> Result<B256, CodecError> {
        if buf.remaining() < 32 {
            return Err(CodecError::EndOfBuffer);
        }
        let mut out = [0u8; 32];
        buf.copy_to_slice(&mut out);
        Ok(B256::from(out))
    }
}

impl FixedSize for BlockId {
    const SIZE: usize = 32;
}

impl FixedSize for TxId {
    const SIZE: usize = 32;
}

impl FixedSize for StateRoot {
    const SIZE: usize = 32;
}

impl Write for BlockId {
    fn write(&self, buf: &mut impl BufMut) {
        Idents::write_b256(&self.0, buf);
    }
}

impl Read for BlockId {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &Self::Cfg) -> Result<Self, CodecError> {
        Ok(Self(Idents::read_b256(buf)?))
    }
}

impl Write for TxId {
    fn write(&self, buf: &mut impl BufMut) {
        Idents::write_b256(&self.0, buf);
    }
}

impl Read for TxId {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &Self::Cfg) -> Result<Self, CodecError> {
        Ok(Self(Idents::read_b256(buf)?))
    }
}

impl Write for StateRoot {
    fn write(&self, buf: &mut impl BufMut) {
        Idents::write_b256(&self.0, buf);
    }
}

impl Read for StateRoot {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &Self::Cfg) -> Result<Self, CodecError> {
        Ok(Self(Idents::read_b256(buf)?))
    }
}

#[cfg(test)]
mod tests {
    use alloy_evm::revm::primitives::{B256, Bytes, keccak256};
    use commonware_codec::{Decode as _, DecodeExt as _, Encode as _, FixedSize as _};

    use super::*;
    use crate::{Block, BlockCfg, Tx, TxCfg};

    fn cfg() -> BlockCfg {
        BlockCfg { max_txs: 64, tx: TxCfg { max_tx_bytes: 1024 } }
    }

    #[test]
    fn test_tx_roundtrip_and_id_stable() {
        let tx = Tx { bytes: Bytes::from(vec![1, 2, 3, 4, 5]) };
        let encoded = tx.encode();
        let decoded =
            Tx::decode_cfg(encoded.clone(), &TxCfg { max_tx_bytes: 1024 }).expect("decode tx");
        assert_eq!(tx, decoded);
        assert_eq!(tx.id(), decoded.id());
        assert_eq!(tx.id(), TxId(keccak256(encoded)));
    }

    #[test]
    fn test_block_roundtrip_and_id_stable() {
        let txs = vec![Tx { bytes: Bytes::new() }, Tx { bytes: Bytes::from(vec![9, 9, 9]) }];
        let block = Block {
            parent: BlockId(B256::from([0xAAu8; 32])),
            height: 7,
            prevrandao: B256::from([0x55u8; 32]),
            state_root: StateRoot(B256::from([0xBBu8; 32])),
            txs,
        };
        let encoded = block.encode();
        let decoded = Block::decode_cfg(encoded.clone(), &cfg()).expect("decode block");
        assert_eq!(block, decoded);
        assert_eq!(block.id(), decoded.id());
        assert_eq!(block.id(), BlockId(keccak256(encoded)));
    }

    #[test]
    fn test_idents_write_and_read_b256_roundtrip() {
        let value = B256::from([0x42u8; 32]);
        let mut buf = Vec::new();
        Idents::write_b256(&value, &mut buf);
        assert_eq!(buf.len(), 32);

        let mut reader = buf.as_slice();
        let decoded = Idents::read_b256(&mut reader).expect("read b256");
        assert_eq!(decoded, value);
    }

    #[test]
    fn test_idents_read_b256_end_of_buffer() {
        let short_buf = [0u8; 16];
        let mut reader = short_buf.as_slice();
        let result = Idents::read_b256(&mut reader);
        assert!(result.is_err());
    }

    #[test]
    fn test_block_id_fixed_size() {
        assert_eq!(BlockId::SIZE, 32);
    }

    #[test]
    fn test_tx_id_fixed_size() {
        assert_eq!(TxId::SIZE, 32);
    }

    #[test]
    fn test_state_root_fixed_size() {
        assert_eq!(StateRoot::SIZE, 32);
    }

    #[test]
    fn test_block_id_ordering() {
        let a = BlockId(B256::from([0x00u8; 32]));
        let b = BlockId(B256::from([0x01u8; 32]));
        let c = BlockId(B256::from([0xFFu8; 32]));

        assert!(a < b);
        assert!(b < c);
        assert!(a < c);
        assert_eq!(a.cmp(&a), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_tx_id_ordering() {
        let a = TxId(B256::from([0x00u8; 32]));
        let b = TxId(B256::from([0x01u8; 32]));
        let c = TxId(B256::from([0xFFu8; 32]));

        assert!(a < b);
        assert!(b < c);
        assert!(a < c);
        assert_eq!(a.cmp(&a), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_block_id_encode_decode_roundtrip() {
        let block_id = BlockId(B256::from([0xABu8; 32]));
        let encoded = block_id.encode();
        assert_eq!(encoded.len(), 32);

        let decoded = BlockId::decode(encoded).expect("decode block id");
        assert_eq!(decoded, block_id);
    }

    #[test]
    fn test_tx_id_encode_decode_roundtrip() {
        let tx_id = TxId(B256::from([0xCDu8; 32]));
        let encoded = tx_id.encode();
        assert_eq!(encoded.len(), 32);

        let decoded = TxId::decode(encoded).expect("decode tx id");
        assert_eq!(decoded, tx_id);
    }

    #[test]
    fn test_state_root_encode_decode_roundtrip() {
        let root = StateRoot(B256::from([0xEFu8; 32]));
        let encoded = root.encode();
        assert_eq!(encoded.len(), 32);

        let decoded = StateRoot::decode(encoded).expect("decode state root");
        assert_eq!(decoded, root);
    }

    #[test]
    fn test_idents_debug_impl() {
        let idents = Idents;
        let debug_str = format!("{idents:?}");
        assert!(debug_str.contains("Idents"));
    }
}
