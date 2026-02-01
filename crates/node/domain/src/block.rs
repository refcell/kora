//! Block types

use alloy_evm::revm::primitives::{B256, keccak256};
use bytes::{Buf, BufMut};
use commonware_codec::{Encode, EncodeSize, Error as CodecError, RangeCfg, Read, ReadExt, Write};
use commonware_cryptography::{Committable, Digestible, Hasher as _, Sha256};

use crate::{BlockId, Idents, StateRoot, Tx, TxCfg};

#[derive(Clone, Copy, Debug)]
/// Configuration used when decoding blocks and their transactions.
pub struct BlockCfg {
    /// Maximum number of transactions that can be encoded in a block.
    pub max_txs: usize,
    /// Per-transaction codec configuration.
    pub tx: TxCfg,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Example block type agreed on by consensus (via its digest).
pub struct Block {
    /// Identifier of the parent block.
    pub parent: BlockId,
    /// Block height (number of committed ancestors).
    pub height: u64,
    /// Seed-derived randomness used for future prevrandao.
    pub prevrandao: B256,
    /// State commitment resulting from this block (pre-commit QMDB root).
    pub state_root: StateRoot,
    /// Transactions included in the block.
    pub txs: Vec<Tx>,
}

impl Block {
    /// Compute the block identifier from its encoded contents.
    pub fn id(&self) -> BlockId {
        BlockId(keccak256(self.encode()))
    }
}

fn digest_for_block_id(id: &BlockId) -> crate::ConsensusDigest {
    let mut hasher = Sha256::default();
    hasher.update(id.0.as_slice());
    hasher.finalize()
}

impl Digestible for Block {
    type Digest = crate::ConsensusDigest;

    fn digest(&self) -> Self::Digest {
        digest_for_block_id(&self.id())
    }
}

impl Committable for Block {
    type Commitment = crate::ConsensusDigest;

    fn commitment(&self) -> Self::Commitment {
        digest_for_block_id(&self.id())
    }
}

impl commonware_consensus::Heightable for Block {
    fn height(&self) -> commonware_consensus::types::Height {
        commonware_consensus::types::Height::new(self.height)
    }
}

impl commonware_consensus::Block for Block {
    fn parent(&self) -> Self::Commitment {
        digest_for_block_id(&self.parent)
    }
}

impl Write for Block {
    fn write(&self, buf: &mut impl BufMut) {
        self.parent.write(buf);
        self.height.write(buf);
        Idents::write_b256(&self.prevrandao, buf);
        self.state_root.write(buf);
        self.txs.write(buf);
    }
}

impl EncodeSize for Block {
    fn encode_size(&self) -> usize {
        self.parent.encode_size()
            + self.height.encode_size()
            + 32
            + self.state_root.encode_size()
            + self.txs.encode_size()
    }
}

impl Read for Block {
    type Cfg = BlockCfg;

    fn read_cfg(buf: &mut impl Buf, cfg: &Self::Cfg) -> Result<Self, CodecError> {
        let parent = BlockId::read(buf)?;
        let height = u64::read(buf)?;
        let prevrandao = Idents::read_b256(buf)?;
        let state_root = StateRoot::read(buf)?;
        let txs = Vec::<Tx>::read_cfg(buf, &(RangeCfg::new(0..=cfg.max_txs), cfg.tx))?;
        Ok(Self { parent, height, prevrandao, state_root, txs })
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Bytes;
    use commonware_codec::Decode;
    use commonware_cryptography::Committable as _;

    use super::*;

    fn default_block_cfg() -> BlockCfg {
        BlockCfg { max_txs: 100, tx: TxCfg { max_tx_bytes: 131072 } }
    }

    fn sample_block() -> Block {
        Block {
            parent: BlockId(B256::repeat_byte(0x01)),
            height: 42,
            prevrandao: B256::repeat_byte(0xab),
            state_root: StateRoot(B256::repeat_byte(0xcd)),
            txs: vec![Tx::new(Bytes::from_static(&[0xde, 0xad, 0xbe, 0xef]))],
        }
    }

    #[test]
    fn block_id_is_deterministic() {
        let block = sample_block();
        let id1 = block.id();
        let id2 = block.id();
        assert_eq!(id1, id2);
    }

    #[test]
    fn block_id_differs_by_height() {
        let block1 = sample_block();
        let mut block2 = sample_block();
        block2.height = 100;
        assert_ne!(block1.id(), block2.id());
    }

    #[test]
    fn block_id_differs_by_parent() {
        let block1 = sample_block();
        let mut block2 = sample_block();
        block2.parent = BlockId(B256::repeat_byte(0xff));
        assert_ne!(block1.id(), block2.id());
    }

    #[test]
    fn block_id_differs_by_txs() {
        let block1 = sample_block();
        let mut block2 = sample_block();
        block2.txs = vec![];
        assert_ne!(block1.id(), block2.id());
    }

    #[test]
    fn block_commitment_matches_digest() {
        let block = sample_block();
        assert_eq!(block.commitment(), block.digest());
    }

    #[test]
    fn block_encode_decode_roundtrip() {
        let block = sample_block();
        let encoded = block.encode();
        let decoded = Block::decode_cfg(encoded, &default_block_cfg()).expect("decode");
        assert_eq!(block, decoded);
    }

    #[test]
    fn block_encode_size_matches_encoded() {
        let block = sample_block();
        assert_eq!(block.encode_size(), block.encode().len());
    }

    #[test]
    fn empty_block_roundtrip() {
        let block = Block {
            parent: BlockId(B256::ZERO),
            height: 0,
            prevrandao: B256::ZERO,
            state_root: StateRoot(B256::ZERO),
            txs: vec![],
        };
        let encoded = block.encode();
        let decoded = Block::decode_cfg(encoded, &default_block_cfg()).expect("decode");
        assert_eq!(block, decoded);
    }

    #[test]
    fn block_heightable() {
        use commonware_consensus::Heightable as _;
        let block = sample_block();
        assert_eq!(block.height().get(), 42);
    }

    #[test]
    fn block_parent_commitment() {
        use commonware_consensus::Block as _;
        let block = sample_block();
        let parent_commitment = block.parent();
        let expected = digest_for_block_id(&block.parent);
        assert_eq!(parent_commitment, expected);
    }
}
