# `kora-runner`

<a href="https://github.com/refcell/kora/actions/workflows/ci.yml"><img src="https://github.com/refcell/kora/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://github.com/refcell/kora/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg" alt="License"></a>

Production node runner for Kora validators. Orchestrates all node components including consensus, execution, storage, and networking.

## Overview

This crate provides the high-level runner that assembles and starts a complete Kora validator node. It integrates:

- **BLS12-381 threshold consensus** via commonware simplex
- **REVM execution** for EVM state transitions
- **QMDB storage** for high-performance state management
- **P2P networking** for validator communication
- **RPC server** for external queries

## Usage

```rust,ignore
use kora_runner::{ProductionRunner, ThresholdScheme, load_threshold_scheme};
use kora_config::NodeConfig;
use kora_domain::BootstrapConfig;

// Load threshold signing scheme from DKG output
let scheme = load_threshold_scheme("/path/to/dkg/output")?;

// Create bootstrap configuration with genesis allocations
let bootstrap = BootstrapConfig::default();

// Create the runner
let runner = ProductionRunner::new(
    scheme,
    1337,           // chain ID
    30_000_000,     // gas limit
    bootstrap,
)
.with_rpc(node_state, "0.0.0.0:8545".parse().unwrap());

// Run as standalone process (blocks until shutdown)
let config = NodeConfig::load("/path/to/config.toml")?;
runner.run_standalone(config)?;
```

### Using with Custom Transport

```rust,ignore
use kora_runner::ProductionRunner;
use kora_service::{NodeRunContext, NodeRunner};

let runner = ProductionRunner::new(scheme, chain_id, gas_limit, bootstrap);

// Build transport and context manually
let ctx = NodeRunContext::new(runtime_context, config, transport);

// Run returns a handle to the ledger service
let ledger = runner.run(ctx).await?;

// Submit transactions
ledger.submit_tx(tx).await;
```

## Architecture

The `ProductionRunner` implements the `NodeRunner` trait and executes the following initialization sequence:

1. **Transport Setup**: Registers validators with the P2P oracle
2. **State Initialization**: Opens QMDB storage with genesis allocations
3. **Ledger Service**: Creates the ledger service for transaction management
4. **Execution Engine**: Initializes REVM executor with chain configuration
5. **Marshal Layer**: Sets up block dissemination and ancestry verification
6. **Simplex Consensus**: Starts the BLS threshold consensus engine

```text
┌─────────────────────────────────────────────────────┐
│                  ProductionRunner                    │
├─────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
│  │   Simplex   │  │   Marshal   │  │     RPC     │ │
│  │  Consensus  │◄─┤   Layer     │  │   Server    │ │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘ │
│         │                │                 │        │
│         ▼                ▼                 ▼        │
│  ┌─────────────────────────────────────────────┐   │
│  │              RevmApplication                 │   │
│  │         (propose & verify blocks)            │   │
│  └──────────────────┬──────────────────────────┘   │
│                     │                               │
│         ┌───────────┴───────────┐                  │
│         ▼                       ▼                  │
│  ┌─────────────┐         ┌─────────────┐          │
│  │    REVM     │         │   Ledger    │          │
│  │  Executor   │         │   Service   │          │
│  └──────┬──────┘         └──────┬──────┘          │
│         │                       │                  │
│         └───────────┬───────────┘                  │
│                     ▼                              │
│              ┌─────────────┐                       │
│              │    QMDB     │                       │
│              │   Storage   │                       │
│              └─────────────┘                       │
└─────────────────────────────────────────────────────┘
```

## Key Types

- `ProductionRunner` - Main production validator runner
- `RevmApplication` - REVM-based consensus application implementing `Application` and `VerifyingApplication`
- `ThresholdScheme` - BLS12-381 threshold signing configuration
- `RunnerError` - Error types for runner operations

## Configuration

The runner is configured through:

| Parameter | Description |
|-----------|-------------|
| `scheme` | BLS12-381 threshold signing scheme from DKG |
| `chain_id` | EVM chain identifier |
| `gas_limit` | Maximum gas per block |
| `bootstrap` | Genesis allocations and bootstrap transactions |
| `rpc_config` | Optional RPC server configuration |

## Threshold Scheme

Load a threshold scheme from DKG ceremony output:

```rust,ignore
use kora_runner::load_threshold_scheme;

// From file path
let scheme = load_threshold_scheme("/path/to/dkg/output.json")?;

// Access participants
let validators = scheme.participants();
```

## Block Lifecycle

### Proposal

When elected as leader, `RevmApplication::propose`:

1. Fetches parent block from ancestry stream
2. Retrieves pending transactions from mempool
3. Executes transactions with REVM
4. Computes new state root
5. Caches execution result in snapshot store
6. Returns block for consensus

### Verification

When validating proposals, `RevmApplication::verify`:

1. Collects unverified blocks from ancestry
2. Verifies parent-to-tip in order
3. Executes transactions and validates state roots
4. Caches verified snapshots for future proposals

## License

This project is licensed under the [MIT License](../../../LICENSE).
