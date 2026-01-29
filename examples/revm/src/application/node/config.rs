use anyhow::Context as _;
use commonware_consensus::simplex::scheme::bls12381_threshold;
use commonware_cryptography::{
    Signer as _,
    bls12381::{
        dkg,
        primitives::{sharing::Mode, variant::MinSig},
    },
    ed25519,
};
use commonware_p2p::simulated;
use commonware_runtime::{Quota, buffer::PoolRef};
use commonware_utils::{N3f1, TryCollect as _, ordered::Set};
use kora_domain::{BlockCfg, PublicKey, TxCfg};
use kora_simplex::{DefaultPool, DefaultQuota};
use kora_transport_sim::SimContext;
use rand::{SeedableRng as _, rngs::StdRng};

pub(crate) type ThresholdScheme = bls12381_threshold::Scheme<PublicKey, MinSig>;

/// Namespace used by simplex votes in this example.
pub(crate) const SIMPLEX_NAMESPACE: &[u8] = b"_COMMONWARE_REVM_SIMPLEX";

pub(crate) use kora_simplex::DEFAULT_MAILBOX_SIZE as MAILBOX_SIZE;

const BLOCK_CODEC_MAX_TXS: usize = 64;
const BLOCK_CODEC_MAX_TX_BYTES: usize = 1024;

pub(crate) type Peer = PublicKey;
pub(crate) type ChannelSender = simulated::Sender<Peer, SimContext>;
pub(crate) type ChannelReceiver = simulated::Receiver<Peer>;

pub(crate) const EPOCH_LENGTH: u64 = u64::MAX;
pub(crate) const PARTITION_PREFIX: &str = "revm";

pub(crate) fn default_quota() -> Quota {
    DefaultQuota::init()
}

pub(crate) fn default_buffer_pool() -> PoolRef {
    DefaultPool::init()
}

/// Default block codec configuration for REVM transactions.
pub(crate) const fn block_codec_cfg() -> BlockCfg {
    BlockCfg { max_txs: BLOCK_CODEC_MAX_TXS, tx: TxCfg { max_tx_bytes: BLOCK_CODEC_MAX_TX_BYTES } }
}

/// Derive deterministic participants and threshold-simplex signing schemes.
pub(crate) fn threshold_schemes(
    seed: u64,
    n: usize,
) -> anyhow::Result<(Vec<PublicKey>, Vec<ThresholdScheme>)> {
    let participants: Set<PublicKey> = (0..n)
        .map(|i| ed25519::PrivateKey::from_seed(seed.wrapping_add(i as u64)).public_key())
        .try_collect()
        .expect("participant public keys are unique");

    let mut rng = StdRng::seed_from_u64(seed);
    let (output, shares) =
        dkg::deal::<MinSig, _, N3f1>(&mut rng, Mode::default(), participants.clone())
            .context("dkg deal failed")?;

    let mut schemes = Vec::with_capacity(n);
    for pk in participants.iter() {
        let share = shares.get_value(pk).expect("share exists").clone();
        let scheme = bls12381_threshold::Scheme::signer(
            SIMPLEX_NAMESPACE,
            participants.clone(),
            output.public().clone(),
            share,
        )
        .context("signer should exist")?;
        schemes.push(scheme);
    }

    Ok((participants.into(), schemes))
}
