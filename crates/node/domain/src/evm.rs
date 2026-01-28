//! EVM-oriented transaction helpers.

use alloy_consensus::{SignableTransaction as _, TxEip1559, TxEnvelope};
use alloy_primitives::{Address, Bytes, Signature, TxKind, U256, keccak256};
use k256::ecdsa::SigningKey;
use sha3::{Digest as _, Keccak256};

use crate::Tx;

/// EVM-specific helpers for transaction construction.
#[derive(Debug)]
pub struct Evm;

impl Evm {
    /// Derive an Ethereum address from a secp256k1 signing key.
    pub fn address_from_key(key: &SigningKey) -> Address {
        let encoded = key.verifying_key().to_encoded_point(false);
        let pubkey = encoded.as_bytes();
        let hash = keccak256(&pubkey[1..]);
        Address::from_slice(&hash[12..])
    }

    /// Sign a simple EIP-1559 transfer transaction and return its encoded bytes.
    #[allow(clippy::too_many_arguments)]
    pub fn sign_eip1559_transfer(
        key: &SigningKey,
        chain_id: u64,
        to: Address,
        value: U256,
        nonce: u64,
        gas_limit: u64,
    ) -> Tx {
        let tx = TxEip1559 {
            chain_id,
            nonce,
            gas_limit,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            to: TxKind::Call(to),
            value,
            access_list: Default::default(),
            input: Bytes::new(),
        };

        let digest = Keccak256::new_with_prefix(tx.encoded_for_signing());
        let (sig, recid) = key.sign_digest_recoverable(digest).expect("sign tx");
        let signature = Signature::from((sig, recid));
        let signed = tx.into_signed(signature);
        let envelope = TxEnvelope::from(signed);
        Tx::new(Bytes::from(alloy_rlp::encode(envelope)))
    }
}
