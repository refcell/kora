# Loadgen

[![MIT License](https://img.shields.io/badge/License-MIT-a78bfa.svg?style=flat&labelColor=1C2C2E)](../../LICENSE)

Load generator for Kora devnet. Generates and submits signed EIP-1559 transactions at high throughput using concurrent execution.

## Usage

```sh
# Start the devnet first
just trusted-devnet

# Run load generator with 1000 transactions
cargo run --release --bin loadgen -- --total-txs 1000

# High concurrency stress test
cargo run --release --bin loadgen -- --total-txs 10000 --concurrency 100 --accounts 50

# Target specific RPC endpoint
cargo run --release --bin loadgen -- --rpc-url http://localhost:8546 --total-txs 5000

# Dry run (test tx signing performance only)
cargo run --release --bin loadgen -- --total-txs 10000 --dry-run
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--rpc-url` | `http://127.0.0.1:8545` | RPC endpoint URL |
| `--accounts` | `10` | Number of sender accounts |
| `--total-txs` | `1000` | Total transactions to send |
| `--concurrency` | `50` | Maximum concurrent in-flight requests |
| `--chain-id` | `1337` | Chain ID for transactions |
| `--dry-run` | `false` | Sign transactions without sending |
| `--verbose` | `false` | Print each transaction hash |

## Notes

The generated accounts need to be funded in the genesis configuration for transactions to succeed. For testing RPC connectivity and mempool acceptance, transactions will be accepted even without funds (they will fail during execution).

Sender addresses are deterministically generated from seed bytes:
- Account 1: seed `[0,0,...,0,1]`
- Account 2: seed `[0,0,...,0,2]`
- etc.

The loadgen outputs the sender addresses at startup so you can fund them in your genesis configuration.

## Performance

The loadgen uses:
- `FuturesUnordered` for concurrent request handling
- Connection pooling via `reqwest`
- Atomic nonce tracking for parallel account access
- Arc-wrapped accounts for thread-safe sharing
