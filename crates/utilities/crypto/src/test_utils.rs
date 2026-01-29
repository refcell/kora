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
use commonware_utils::{N3f1, TryCollect as _, ordered::Set};
use kora_domain::PublicKey;
use rand::{SeedableRng as _, rngs::StdRng};

/// Threshold BLS signing scheme using MinSig variant.
pub type ThresholdScheme = bls12381_threshold::Scheme<PublicKey, MinSig>;

const SIMPLEX_NAMESPACE: &[u8] = b"_COMMONWARE_REVM_SIMPLEX";

/// Generate deterministic threshold BLS signing schemes for testing.
pub fn threshold_schemes(
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
