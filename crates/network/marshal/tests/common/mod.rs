//! Shared test utilities for kora-marshal integration tests.

use commonware_codec::{FixedSize, Read, ReadExt, Write};
use commonware_consensus::{Block as BlockTrait, Heightable, types::Height};
use commonware_cryptography::{
    Committable, Digestible, Hasher as _,
    sha256::{Digest as Sha256Digest, Sha256},
};

/// A test block implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Block {
    parent: Sha256Digest,
    height: Height,
    timestamp: u64,
    digest: Sha256Digest,
}

impl Block {
    /// Creates a new block with the given parent, height, and timestamp.
    pub(crate) fn new(parent: Sha256Digest, height: Height, timestamp: u64) -> Self {
        let mut hasher = Sha256::default();
        hasher.update(parent.as_ref());
        hasher.update(&height.get().to_le_bytes());
        hasher.update(&timestamp.to_le_bytes());
        let digest = hasher.finalize();
        Self { parent, height, timestamp, digest }
    }
}

impl Heightable for Block {
    fn height(&self) -> Height {
        self.height
    }
}

impl Digestible for Block {
    type Digest = Sha256Digest;

    fn digest(&self) -> Self::Digest {
        self.digest
    }
}

impl Committable for Block {
    type Commitment = Sha256Digest;

    fn commitment(&self) -> Self::Commitment {
        self.digest
    }
}

impl BlockTrait for Block {
    fn parent(&self) -> Self::Digest {
        self.parent
    }
}

impl FixedSize for Block {
    const SIZE: usize = 32 + 8 + 8 + 32; // parent + height + timestamp + digest
}

impl Write for Block {
    fn write(&self, buf: &mut impl bytes::BufMut) {
        self.parent.write(buf);
        self.height.get().write(buf);
        self.timestamp.write(buf);
        self.digest.write(buf);
    }
}

impl Read for Block {
    type Cfg = ();

    fn read_cfg(
        buf: &mut impl bytes::Buf,
        _cfg: &Self::Cfg,
    ) -> Result<Self, commonware_codec::Error> {
        let parent = Sha256Digest::read(buf)?;
        let height = Height::new(u64::read(buf)?);
        let timestamp = u64::read(buf)?;
        let digest = Sha256Digest::read(buf)?;
        Ok(Self { parent, height, timestamp, digest })
    }
}
