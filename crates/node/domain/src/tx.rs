//! Transactions

use alloy_primitives::{Bytes, keccak256};
use bytes::{Buf, BufMut};
use commonware_codec::{Encode, EncodeSize, Error as CodecError, RangeCfg, Read, Write};

use super::TxId;

#[derive(Clone, Copy, Debug)]
/// Configuration used when decoding transactions from bytes.
pub struct TxCfg {
    /// Maximum encoded transaction size accepted by the codec.
    pub max_tx_bytes: usize,
}

/// Raw transaction bytes for the example.
///
/// This is expected to contain a signed Ethereum transaction envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tx {
    /// Encoded transaction bytes.
    pub bytes: Bytes,
}

impl Tx {
    /// Compute the transaction identifier from its encoded contents.
    pub fn id(&self) -> TxId {
        TxId(keccak256(self.encode()))
    }

    /// Create a new transaction from encoded bytes.
    pub const fn new(bytes: Bytes) -> Self {
        Self { bytes }
    }
}

impl Write for Tx {
    fn write(&self, buf: &mut impl BufMut) {
        self.bytes.as_ref().write(buf);
    }
}

impl EncodeSize for Tx {
    fn encode_size(&self) -> usize {
        self.bytes.as_ref().encode_size()
    }
}

impl Read for Tx {
    type Cfg = TxCfg;

    fn read_cfg(buf: &mut impl Buf, cfg: &Self::Cfg) -> Result<Self, CodecError> {
        let data = Vec::<u8>::read_cfg(buf, &(RangeCfg::new(0..=cfg.max_tx_bytes), ()))?;
        Ok(Self { bytes: Bytes::from(data) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_codec::Decode;

    fn default_tx_cfg() -> TxCfg {
        TxCfg { max_tx_bytes: 131072 }
    }

    #[test]
    fn tx_id_is_deterministic() {
        let tx = Tx::new(Bytes::from_static(&[0x01, 0x02, 0x03]));
        let id1 = tx.id();
        let id2 = tx.id();
        assert_eq!(id1, id2);
    }

    #[test]
    fn tx_id_differs_by_content() {
        let tx1 = Tx::new(Bytes::from_static(&[0x01, 0x02]));
        let tx2 = Tx::new(Bytes::from_static(&[0x01, 0x03]));
        assert_ne!(tx1.id(), tx2.id());
    }

    #[test]
    fn tx_encode_decode_roundtrip() {
        let tx = Tx::new(Bytes::from_static(&[0xde, 0xad, 0xbe, 0xef]));
        let encoded = tx.encode();
        let decoded = Tx::decode_cfg(encoded, &default_tx_cfg()).expect("decode");
        assert_eq!(tx, decoded);
    }

    #[test]
    fn tx_encode_size_matches_encoded() {
        let tx = Tx::new(Bytes::from_static(&[0x01, 0x02, 0x03, 0x04, 0x05]));
        assert_eq!(tx.encode_size(), tx.encode().len());
    }

    #[test]
    fn empty_tx_roundtrip() {
        let tx = Tx::new(Bytes::new());
        let encoded = tx.encode();
        let decoded = Tx::decode_cfg(encoded, &default_tx_cfg()).expect("decode");
        assert_eq!(tx, decoded);
    }

    #[test]
    fn large_tx_roundtrip() {
        let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let tx = Tx::new(Bytes::from(data));
        let encoded = tx.encode();
        let decoded = Tx::decode_cfg(encoded, &default_tx_cfg()).expect("decode");
        assert_eq!(tx, decoded);
    }
}
