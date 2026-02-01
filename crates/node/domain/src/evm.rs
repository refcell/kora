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

#[cfg(test)]
mod tests {
    use alloy_consensus::Transaction;
    use alloy_eips::eip2718::Decodable2718;

    use super::*;

    fn signing_key_from_seed(seed: u8) -> SigningKey {
        let mut secret = [0u8; 32];
        secret[31] = seed;
        SigningKey::from_bytes((&secret).into()).expect("valid key")
    }

    #[test]
    fn address_from_key_is_deterministic() {
        let key = signing_key_from_seed(1);
        let addr1 = Evm::address_from_key(&key);
        let addr2 = Evm::address_from_key(&key);
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn address_from_key_differs_by_key() {
        let key1 = signing_key_from_seed(1);
        let key2 = signing_key_from_seed(2);
        let addr1 = Evm::address_from_key(&key1);
        let addr2 = Evm::address_from_key(&key2);
        assert_ne!(addr1, addr2);
    }

    #[test]
    fn address_from_key_has_correct_length() {
        let key = signing_key_from_seed(42);
        let addr = Evm::address_from_key(&key);
        assert_eq!(addr.len(), 20);
    }

    #[test]
    fn sign_eip1559_transfer_produces_valid_envelope() {
        let key = signing_key_from_seed(1);
        let to = Address::repeat_byte(0xab);
        let value = U256::from(1000);

        let tx = Evm::sign_eip1559_transfer(&key, 1, to, value, 0, 21000);

        let envelope =
            TxEnvelope::decode_2718(&mut tx.bytes.as_ref()).expect("valid envelope encoding");

        assert!(matches!(envelope, TxEnvelope::Eip1559(_)));
    }

    #[test]
    fn sign_eip1559_transfer_correct_chain_id() {
        let key = signing_key_from_seed(1);
        let to = Address::repeat_byte(0xab);
        let chain_id = 42u64;

        let tx = Evm::sign_eip1559_transfer(&key, chain_id, to, U256::ZERO, 0, 21000);

        let envelope =
            TxEnvelope::decode_2718(&mut tx.bytes.as_ref()).expect("valid envelope encoding");
        assert_eq!(envelope.chain_id(), Some(chain_id));
    }

    #[test]
    fn sign_eip1559_transfer_correct_nonce() {
        let key = signing_key_from_seed(1);
        let to = Address::repeat_byte(0xab);
        let nonce = 123u64;

        let tx = Evm::sign_eip1559_transfer(&key, 1, to, U256::ZERO, nonce, 21000);

        let envelope =
            TxEnvelope::decode_2718(&mut tx.bytes.as_ref()).expect("valid envelope encoding");
        assert_eq!(envelope.nonce(), nonce);
    }

    #[test]
    fn sign_eip1559_transfer_correct_gas_limit() {
        let key = signing_key_from_seed(1);
        let to = Address::repeat_byte(0xab);
        let gas_limit = 50000u64;

        let tx = Evm::sign_eip1559_transfer(&key, 1, to, U256::ZERO, 0, gas_limit);

        let envelope =
            TxEnvelope::decode_2718(&mut tx.bytes.as_ref()).expect("valid envelope encoding");
        assert_eq!(envelope.gas_limit(), gas_limit);
    }

    #[test]
    fn sign_eip1559_transfer_correct_value() {
        let key = signing_key_from_seed(1);
        let to = Address::repeat_byte(0xab);
        let value = U256::from(999_999);

        let tx = Evm::sign_eip1559_transfer(&key, 1, to, value, 0, 21000);

        let envelope =
            TxEnvelope::decode_2718(&mut tx.bytes.as_ref()).expect("valid envelope encoding");
        assert_eq!(envelope.value(), value);
    }

    #[test]
    fn sign_eip1559_transfer_correct_recipient() {
        let key = signing_key_from_seed(1);
        let to = Address::repeat_byte(0xcd);

        let tx = Evm::sign_eip1559_transfer(&key, 1, to, U256::ZERO, 0, 21000);

        let envelope =
            TxEnvelope::decode_2718(&mut tx.bytes.as_ref()).expect("valid envelope encoding");
        assert_eq!(envelope.to(), Some(to));
    }

    #[test]
    fn sign_eip1559_transfer_different_params_produce_different_txs() {
        let key = signing_key_from_seed(1);
        let to = Address::repeat_byte(0xab);

        let tx1 = Evm::sign_eip1559_transfer(&key, 1, to, U256::from(100), 0, 21000);
        let tx2 = Evm::sign_eip1559_transfer(&key, 1, to, U256::from(200), 0, 21000);

        assert_ne!(tx1.bytes, tx2.bytes);
    }

    #[test]
    fn sign_eip1559_transfer_different_nonces_produce_different_txs() {
        let key = signing_key_from_seed(1);
        let to = Address::repeat_byte(0xab);

        let tx1 = Evm::sign_eip1559_transfer(&key, 1, to, U256::ZERO, 0, 21000);
        let tx2 = Evm::sign_eip1559_transfer(&key, 1, to, U256::ZERO, 1, 21000);

        assert_ne!(tx1.bytes, tx2.bytes);
    }

    #[test]
    fn signed_tx_has_recoverable_signature() {
        let key = signing_key_from_seed(5);
        let to = Address::repeat_byte(0xef);
        let sender = Evm::address_from_key(&key);

        let tx = Evm::sign_eip1559_transfer(&key, 1, to, U256::from(500), 0, 21000);

        let envelope =
            TxEnvelope::decode_2718(&mut tx.bytes.as_ref()).expect("valid envelope encoding");

        let recovered = envelope.recover_signer().expect("recover signer");
        assert_eq!(recovered, sender);
    }
}
