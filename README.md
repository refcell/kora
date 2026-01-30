<p align="center">
    <img src="./assets/logo.png" alt="Kora" width="300">
</p>

<h1 align="center">Kora</h1>

<h4 align="center">
    A minimal commonware + revm execution client. Built in Rust.
</h4>

<p align="center">
  <img src="https://img.shields.io/crates/msrv/kora?style=flat&labelColor=1C2C2E&color=fbbf24&logo=rust&logoColor=white" alt="MSRV">
  <a href="https://github.com/refcell/kora/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/refcell/kora/ci.yml?style=flat&labelColor=1C2C2E&label=ci&logo=GitHub%20Actions&logoColor=white" alt="CI"></a>
  <a href="https://github.com/refcell/kora/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?style=flat&labelColor=1C2C2E&color=a78bfa&logo=googledocs&logoColor=white" alt="License"></a>
</p>

<p align="center">
  <a href="#whats-kora">What's Kora?</a> •
  <a href="#usage">Usage</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#contributing">Contributing</a> •
  <a href="#license">License</a>
</p>

## Demo

https://github.com/user-attachments/assets/8ea05477-039d-4dd2-a7f3-660f179299f7

> [!CAUTION]
> Kora is pre-alpha software. Do not expect this to run. Do not run this in production.

> [!IMPORTANT]
> Kora uses BLS12-381 threshold consensus via [commonware](https://github.com/commonwarexyz/monorepo), [REVM](https://github.com/bluealloy/revm) for EVM execution, and [QMDB](https://github.com/commonwarexyz/monorepo/tree/main/storage) for state storage.

## What's Kora?

Kora is a minimal, high-performance execution client built entirely in Rust. It combines commonware's BLS12-381 threshold consensus with REVM for EVM execution and QMDB for efficient state storage. The architecture is modular with distinct layers for consensus (BLS12-381 threshold signatures via commonware simplex), execution (EVM state transitions powered by REVM), storage (high-performance state management with QMDB), and networking (P2P transport and message marshaling).

## Why?

Existing options for building EVM-compatible nodes on commonware are limited. The [tempo node](https://github.com/commonwarexyz/monorepo/tree/main/examples/tempo) uses reth, which is heavy and comes with opinionated Ethereum traits and abstractions that may not suit all use cases. The [revm commonware example](https://github.com/commonwarexyz/monorepo/pull/2495) is limited to a mock simulation without real networking or persistence. There isn't a public, lightweight node available that connects revm directly to commonware with full consensus and storage. Kora fills this gap by providing a minimal, production-oriented node that integrates revm with commonware simplex consensus and QMDB storage.

## Usage

Start the devnet with interactive DKG (Distributed Key Generation):

```sh
just devnet
```

> [!TIP]
> See the [Justfile](./Justfile) for other useful commands.

## Architecture

The devnet runs in three phases. Phase 0 generates ed25519 identity keys for each validator node. Phase 1 is the DKG ceremony, an interactive threshold key generation process using Ed25519 simplex consensus where validators collaborate to generate a shared BLS12-381 threshold key. Phase 2 launches full validator nodes running BLS12-381 threshold consensus, the REVM execution engine, and QMDB state storage.

Observability is provided through Prometheus metrics with Grafana dashboards for monitoring node health, consensus performance, and execution metrics.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
