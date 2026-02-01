# `kora-rpc`

<a href="https://github.com/refcell/kora/actions/workflows/ci.yml"><img src="https://github.com/refcell/kora/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://github.com/refcell/kora/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg" alt="License"></a>

RPC server for Kora nodes. Provides HTTP status endpoints and a full Ethereum JSON-RPC 2.0 API implementation.

## Overview

This crate implements the RPC layer for Kora nodes, exposing:

- **Ethereum JSON-RPC API** (`eth_*`, `net_*`, `web3_*`) for wallet and tooling compatibility
- **Kora-specific API** (`kora_*`) for node status and consensus information
- **HTTP endpoints** for health checks and node monitoring

## Usage

```rust,ignore
use kora_rpc::{RpcServer, RpcServerConfig, NodeState, StateProvider};
use std::net::SocketAddr;

// Create server configuration
let config = RpcServerConfig::single_addr(
    "127.0.0.1:8545".parse().unwrap(),
    1337, // chain ID
);

// Create node state for status reporting
let state = NodeState::new("validator-1".to_string());

// Start the server with a custom state provider
let server = RpcServer::with_state_provider(
    state,
    config.http_addr,
    config.chain_id,
    my_state_provider,
)
.with_cors(CorsConfig::permissive())
.start();

// Server runs in background, wait for shutdown
server.stopped().await;
```

## Ethereum JSON-RPC Methods

The following standard Ethereum methods are supported:

| Method | Description |
|--------|-------------|
| `eth_chainId` | Returns the chain ID |
| `eth_blockNumber` | Returns the current block number |
| `eth_getBalance` | Returns account balance |
| `eth_getTransactionCount` | Returns account nonce |
| `eth_getCode` | Returns contract bytecode |
| `eth_getStorageAt` | Returns storage slot value |
| `eth_sendRawTransaction` | Submits a signed transaction |
| `eth_call` | Executes a call without creating a transaction |
| `eth_estimateGas` | Estimates gas for a transaction |
| `eth_getBlockByNumber` | Returns block by number |
| `eth_getBlockByHash` | Returns block by hash |
| `eth_getTransactionByHash` | Returns transaction by hash |
| `eth_getTransactionReceipt` | Returns transaction receipt |
| `eth_gasPrice` | Returns current gas price |
| `eth_maxPriorityFeePerGas` | Returns max priority fee |
| `eth_feeHistory` | Returns fee history |
| `eth_accounts` | Returns accounts (empty for non-wallet nodes) |
| `eth_protocolVersion` | Returns protocol version |
| `eth_syncing` | Returns sync status |
| `net_version` | Returns network ID |
| `net_listening` | Returns listening status |
| `net_peerCount` | Returns peer count |
| `web3_clientVersion` | Returns client version |
| `web3_sha3` | Returns Keccak-256 hash |

## Kora-Specific Methods

| Method | Description |
|--------|-------------|
| `kora_nodeStatus` | Returns node status including consensus info |

## HTTP Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /health` | Health check (returns "ok") |
| `GET /status` | Returns detailed node status as JSON |

## Configuration

### RpcServerConfig

```rust,ignore
use kora_rpc::{RpcServerConfig, CorsConfig, RateLimitConfig};

let config = RpcServerConfig::new(
    "127.0.0.1:8080".parse().unwrap(),  // HTTP address
    "127.0.0.1:8545".parse().unwrap(),  // JSON-RPC address
    1337,                                // Chain ID
)
.with_cors_origins(vec!["http://localhost:3000".to_string()])
.with_rate_limit(100)  // requests per second
.with_max_connections(200);
```

### CORS Configuration

```rust,ignore
// Development (allow all origins - not for production)
let cors = CorsConfig::permissive();

// Production (specific origins)
let cors = CorsConfig {
    allowed_origins: vec!["https://app.example.com".to_string()],
    allowed_methods: vec!["GET".to_string(), "POST".to_string()],
    allowed_headers: vec!["Content-Type".to_string()],
    max_age: 3600,
};

// Disabled
let cors = CorsConfig::none();
```

## Key Types

- `RpcServer` - Combined HTTP and JSON-RPC server
- `JsonRpcServer` - Standalone JSON-RPC server without HTTP endpoints
- `RpcServerConfig` - Server configuration
- `StateProvider` - Trait for providing chain state to RPC methods
- `EthApiServer` - Ethereum JSON-RPC API trait
- `KoraApiServer` - Kora-specific API trait
- `NodeState` - Node status container
- `NoopStateProvider` - Default provider returning empty/zero values

## Custom State Providers

Implement `StateProvider` to connect RPC methods to your storage layer:

```rust,ignore
use kora_rpc::{StateProvider, RpcError, BlockNumberOrTag, RpcBlock};
use alloy_primitives::{Address, U256};
use async_trait::async_trait;

struct MyStateProvider {
    // Your storage backend
}

#[async_trait]
impl StateProvider for MyStateProvider {
    async fn balance(
        &self,
        address: Address,
        block: Option<BlockNumberOrTag>,
    ) -> Result<U256, RpcError> {
        // Query your storage
        Ok(U256::from(1000))
    }

    // Implement other required methods...
}
```

## License

This project is licensed under the [MIT License](../../../LICENSE).
