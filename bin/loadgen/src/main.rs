#![doc = include_str!("../README.md")]
//! Load generator for Kora devnet.
//!
//! Sends high volumes of EIP-1559 transactions to stress test the network.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use alloy_consensus::{SignableTransaction as _, TxEip1559, TxEnvelope};
use alloy_eips::eip2718::Encodable2718;
use alloy_primitives::{Address, Bytes, Signature, TxKind, U256, keccak256};
use clap::Parser;
use eyre::Result;
use futures::stream::{FuturesUnordered, StreamExt};
use k256::ecdsa::SigningKey;
use sha3::{Digest as _, Keccak256};
use tracing::{error, info, warn};

/// Load generator CLI.
#[derive(Parser, Debug)]
#[command(name = "loadgen", about = "Load generator for Kora devnet")]
struct Args {
    /// RPC endpoint URL.
    #[arg(long, default_value = "http://127.0.0.1:8545")]
    rpc_url: String,

    /// Number of accounts to use for sending transactions.
    #[arg(long, default_value = "10")]
    accounts: usize,

    /// Total number of transactions to send.
    #[arg(long, default_value = "1000")]
    total_txs: u64,

    /// Maximum number of concurrent in-flight requests.
    #[arg(long, default_value = "50")]
    concurrency: usize,

    /// Chain ID.
    #[arg(long, default_value = "1337")]
    chain_id: u64,

    /// Dry run (don't actually send transactions).
    #[arg(long)]
    dry_run: bool,

    /// Print each transaction hash.
    #[arg(long)]
    verbose: bool,
}

/// Account with signing key and nonce tracker.
struct Account {
    key: SigningKey,
    address: Address,
    nonce: AtomicU64,
}

impl Account {
    fn new(seed: u8) -> Self {
        let mut secret = [0u8; 32];
        secret[31] = seed;
        let key = SigningKey::from_bytes((&secret).into()).expect("valid key");
        let address = address_from_key(&key);
        Self { key, nonce: AtomicU64::new(0), address }
    }

    fn next_nonce(&self) -> u64 {
        self.nonce.fetch_add(1, Ordering::Relaxed)
    }
}

fn address_from_key(key: &SigningKey) -> Address {
    let encoded = key.verifying_key().to_encoded_point(false);
    let pubkey = encoded.as_bytes();
    let hash = keccak256(&pubkey[1..]);
    Address::from_slice(&hash[12..])
}

fn sign_eip1559_transfer(
    key: &SigningKey,
    chain_id: u64,
    to: Address,
    value: U256,
    nonce: u64,
    gas_limit: u64,
) -> Bytes {
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
    let mut raw_bytes = Vec::new();
    envelope.encode_2718(&mut raw_bytes);
    Bytes::from(raw_bytes)
}

/// HTTP client for RPC calls.
#[derive(Clone)]
struct RpcClient {
    client: reqwest::Client,
    url: String,
}

impl RpcClient {
    fn new(url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(100)
            .build()
            .expect("build http client");
        Self { client, url }
    }

    async fn send_raw_transaction(&self, raw_tx: &[u8]) -> Result<String> {
        let hex_tx = format!("0x{}", hex::encode(raw_tx));

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_sendRawTransaction",
            "params": [hex_tx],
            "id": 1
        });

        let resp = self.client.post(&self.url).json(&body).send().await?;

        let json: serde_json::Value = resp.json().await?;

        if let Some(error) = json.get("error") {
            eyre::bail!("RPC error: {}", error);
        }

        Ok(json["result"].as_str().unwrap_or("").to_string())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    info!(
        rpc_url = %args.rpc_url,
        accounts = args.accounts,
        total_txs = args.total_txs,
        concurrency = args.concurrency,
        chain_id = args.chain_id,
        dry_run = args.dry_run,
        "Starting load generator"
    );

    let accounts: Vec<Arc<Account>> =
        (1..=args.accounts).map(|i| Arc::new(Account::new(i as u8))).collect();

    info!("Sender addresses (fund these with ETH):");
    for acc in &accounts {
        info!("  {}", acc.address);
    }

    let receiver = Address::repeat_byte(0xBB);
    let transfer_amount = U256::from(1u64);
    let gas_limit = 21_000u64;

    let client = RpcClient::new(args.rpc_url.clone());

    let success_count = Arc::new(AtomicU64::new(0));
    let failure_count = Arc::new(AtomicU64::new(0));

    let start = Instant::now();

    if args.dry_run {
        for i in 0..args.total_txs {
            let account = &accounts[i as usize % accounts.len()];
            let nonce = account.next_nonce();
            let _tx = sign_eip1559_transfer(
                &account.key,
                args.chain_id,
                receiver,
                transfer_amount,
                nonce,
                gas_limit,
            );
            success_count.fetch_add(1, Ordering::Relaxed);
            if (i + 1) % 1000 == 0 {
                info!(tx = i + 1, "Dry run progress");
            }
        }
    } else {
        let mut futures = FuturesUnordered::new();

        for i in 0..args.total_txs {
            let account = accounts[i as usize % accounts.len()].clone();
            let client = client.clone();
            let success = success_count.clone();
            let failure = failure_count.clone();
            let verbose = args.verbose;

            let nonce = account.next_nonce();
            let tx = sign_eip1559_transfer(
                &account.key,
                args.chain_id,
                receiver,
                transfer_amount,
                nonce,
                gas_limit,
            );

            let fut = async move {
                match client.send_raw_transaction(&tx).await {
                    Ok(hash) => {
                        success.fetch_add(1, Ordering::Relaxed);
                        if verbose {
                            info!(nonce, hash = %hash, "tx sent");
                        }
                    }
                    Err(e) => {
                        failure.fetch_add(1, Ordering::Relaxed);
                        warn!(nonce, error = %e, "tx failed");
                    }
                }
            };

            futures.push(fut);

            // Limit concurrency by waiting when we hit the limit
            if futures.len() >= args.concurrency {
                futures.next().await;
            }
        }

        // Drain remaining futures
        while futures.next().await.is_some() {}
    }

    let elapsed = start.elapsed();
    let success = success_count.load(Ordering::Relaxed);
    let failure = failure_count.load(Ordering::Relaxed);
    let tps =
        if elapsed.as_secs_f64() > 0.0 { success as f64 / elapsed.as_secs_f64() } else { 0.0 };

    info!(
        sent = success + failure,
        success,
        failed = failure,
        elapsed_secs = format!("{:.2}", elapsed.as_secs_f64()),
        tps = format!("{:.2}", tps),
        "Load generation complete"
    );

    if failure > 0 {
        error!(failed = failure, "Some transactions failed");
    }

    Ok(())
}
