use std::{num::NonZeroU32, path::Path};

use commonware_codec::{Read, ReadExt};
use commonware_consensus::simplex::scheme::bls12381_threshold::vrf as bls12381_threshold;
use commonware_cryptography::{
    bls12381::primitives::{
        group::Share,
        sharing::{ModeVersion, Sharing},
        variant::MinSig,
    },
    ed25519,
};
use commonware_utils::ordered::Set;
use kora_dkg::DkgOutput;

/// BLS12-381 threshold signature scheme used for consensus.
pub type ThresholdScheme = bls12381_threshold::Scheme<ed25519::PublicKey, MinSig>;

const SIMPLEX_NAMESPACE: &[u8] = b"_COMMONWARE_KORA_SIMPLEX";

/// Load a threshold signing scheme from DKG output files.
pub fn load_threshold_scheme(data_dir: &Path) -> anyhow::Result<ThresholdScheme> {
    let output = DkgOutput::load(data_dir)?;

    let participants: Vec<ed25519::PublicKey> = output
        .participant_keys
        .iter()
        .map(|k| {
            ed25519::PublicKey::read(&mut k.as_slice())
                .map_err(|e| anyhow::anyhow!("failed to decode participant key: {:?}", e))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let n = participants.len();
    let n_cfg =
        NonZeroU32::new(n as u32).ok_or_else(|| anyhow::anyhow!("participants cannot be empty"))?;

    let participants_set = Set::from_iter_dedup(participants);

    let group_poly = Sharing::<MinSig>::read_cfg(
        &mut output.public_polynomial.as_slice(),
        &(n_cfg, ModeVersion::v0()),
    )
    .map_err(|e| anyhow::anyhow!("failed to decode public polynomial: {:?}", e))?;

    let share = Share::read_cfg(&mut output.share_secret.as_slice(), &())
        .map_err(|e| anyhow::anyhow!("failed to decode share: {:?}", e))?;

    let scheme =
        bls12381_threshold::Scheme::signer(SIMPLEX_NAMESPACE, participants_set, group_poly, share)
            .ok_or_else(|| anyhow::anyhow!("failed to create signer: share public key mismatch"))?;

    Ok(scheme)
}
