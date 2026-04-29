# Kora Docker Devnet

This directory contains Docker configurations for running a local Kora devnet with either:
- **Interactive DKG** (production-like, no single party learns the master secret)
- **Trusted Dealer DKG** (fast, insecure, for local development)

The devnet also starts one **secondary peer**. Secondary peers are authorized P2P nodes
that follow validator traffic but are not consensus participants and do not receive DKG
shares.

## Quick Start

From the repository root:

```bash
# Production-like devnet with interactive DKG ceremony
just devnet

# Fast devnet with trusted dealer (for quick iteration)
just trusted-devnet
```

### Interactive DKG (default)
1. Build the Docker image
2. Generate validator identity keys (init-setup)
3. Run interactive DKG ceremony across 4 nodes
4. Start 4 validator nodes with threshold BLS consensus
5. Start 1 secondary peer that joins through the validators
6. Start Prometheus and Grafana for observability (optional)

### Trusted Dealer (fast)
1. Build the Docker image
2. Generate validator identity keys and run trusted dealer DKG (init-config)
3. Start 4 validator nodes with threshold BLS consensus
4. Start 1 secondary peer that joins through the validators
5. Start Prometheus and Grafana for observability (optional)

## Commands

Run from repository root (`just <cmd>`) or from `docker/` directory (`just <cmd>`):

### From Repository Root

| Command | Description |
|---------|-------------|
| `just devnet` | Start devnet with interactive DKG (production-like) |
| `just trusted-devnet` | Start devnet with trusted dealer DKG (fast, insecure) |
| `just devnet-down` | Stop all containers (preserves state) |
| `just devnet-reset` | Stop and delete all state (fresh DKG on next start) |
| `just devnet-logs` | Stream validator logs |
| `just devnet-status` | Show container status and endpoints |
| `just devnet-stats` | Live devnet monitoring dashboard |
| `just docker-build` | Build docker images |

### From docker/ Directory

| Command | Description |
|---------|-------------|
| `just build` | Build the Docker image |
| `just build-fresh` | Build the Docker image without cache |
| `just devnet` | Start devnet with interactive DKG (production-like) |
| `just trusted-devnet` | Start devnet with trusted dealer DKG (fast, insecure) |
| `just devnet-minimal` | Start devnet without observability stack |
| `just down` | Stop all containers (preserves state) |
| `just reset` | Stop and delete all state (fresh DKG on next start) |
| `just restart` | Stop and restart the devnet |
| `just restart-validators` | Restart only validator nodes |
| `just status` | Show container status and endpoints |
| `just stats` | Live devnet monitoring dashboard |
| `just logs` | Stream validator logs |
| `just logs-dkg` | Stream DKG node logs |
| `just logs-node <node>` | Stream logs for a specific node |
| `just exec <node>` | Open a shell in a running container |
| `just redo-dkg` | Re-run DKG ceremony (keeps other state) |
| `just validate` | Validate the compose file |
| `just lint` | Lint the Dockerfile with hadolint |

## Architecture

### Interactive DKG Flow (default)
```
┌─────────────────────────────────────────────────────────────────┐
│                   Kora Devnet (Interactive DKG)                  │
├─────────────────────────────────────────────────────────────────┤
│  init-setup (runs once)                                          │
│    - Generates ed25519 identity keys                             │
│    - Creates peers.json and genesis.json                         │
│    - Does NOT generate threshold shares                          │
├─────────────────────────────────────────────────────────────────┤
│  DKG Ceremony (dkg-node0..3, one-shot jobs)                      │
│    - Interactive Joint-Feldman DKG protocol                      │
│    - Each node contributes to key generation                     │
│    - No single node learns the master secret                     │
│    - Outputs: output.json + share.key per node                   │
├─────────────────────────────────────────────────────────────────┤
│  Validators (validator-node0..3)                                 │
│    - BLS12-381 threshold consensus                               │
│    - REVM execution                                              │
│    - QMDB state storage                                          │
├─────────────────────────────────────────────────────────────────┤
│  Secondary peer (secondary-node0)                                │
│    - Authenticated P2P participant                               │
│    - Tracks validator traffic without joining consensus          │
├─────────────────────────────────────────────────────────────────┤
│  Observability (optional): prometheus + grafana                  │
│    - Prometheus: http://localhost:9090                           │
│    - Grafana: http://localhost:3000 (admin/admin)                │
└─────────────────────────────────────────────────────────────────┘
```

### Trusted Dealer Flow (fast)
```
┌─────────────────────────────────────────────────────────────────┐
│                  Kora Devnet (Trusted Dealer)                    │
├─────────────────────────────────────────────────────────────────┤
│  init-config (runs once)                                         │
│    - Generates ed25519 identity keys                             │
│    - Runs trusted dealer DKG (single party generates all shares) │
│    - Creates peers.json and genesis.json                         │
│    - Outputs: output.json + share.key per node                   │
├─────────────────────────────────────────────────────────────────┤
│  Validators (validator-node0..3)                                 │
│    - BLS12-381 threshold consensus                               │
│    - REVM execution                                              │
│    - QMDB state storage                                          │
├─────────────────────────────────────────────────────────────────┤
│  Secondary peer (secondary-node0)                                │
│    - Authenticated P2P participant                               │
│    - Tracks validator traffic without joining consensus          │
├─────────────────────────────────────────────────────────────────┤
│  Observability (optional): prometheus + grafana                  │
│    - Prometheus: http://localhost:9090                           │
│    - Grafana: http://localhost:3000 (admin/admin)                │
└─────────────────────────────────────────────────────────────────┘
```

**Security Note:** The trusted dealer flow is only for local development. Use the interactive DKG flow for production-like testing.

## Endpoints

| Service | Port | Description |
|---------|------|-------------|
| validator-node0 P2P | 30400 | P2P networking |
| validator-node1 P2P | 30401 | P2P networking |
| validator-node2 P2P | 30402 | P2P networking |
| validator-node3 P2P | 30403 | P2P networking |
| secondary-node0 P2P | 30500 | Secondary P2P networking |
| validator-node0 RPC | 8545 | JSON-RPC endpoint |
| validator-node1 RPC | 8546 | JSON-RPC endpoint |
| validator-node2 RPC | 8547 | JSON-RPC endpoint |
| validator-node3 RPC | 8548 | JSON-RPC endpoint |
| validator-node0 Metrics | 9000 | Prometheus metrics |
| validator-node1 Metrics | 9001 | Prometheus metrics |
| validator-node2 Metrics | 9002 | Prometheus metrics |
| validator-node3 Metrics | 9003 | Prometheus metrics |
| Prometheus | 9090 | Metrics aggregation |
| Grafana | 3000 | Dashboards |

## Configuration

Environment variables (set in `.env` or export):

| Variable | Default | Description |
|----------|---------|-------------|
| `CHAIN_ID` | 1337 | Chain identifier |
| `RUST_LOG` | info | Log level (trace, debug, info, warn, error) |
| `COMPOSE_PROFILES` | observability | Comma-separated profiles (observability, distributed-dkg) |
| `VALIDATOR_INDEX` | - | Node index (0-3), set per container |
| `IS_BOOTSTRAP` | - | Whether node is bootstrap node |
| `BOOTSTRAP_PEERS` | - | Bootstrap peer addresses |
| `HEALTHCHECK_MODE` | - | Health check mode (dkg, ready) |

## Secondary Peers

Kora follows Commonware's primary/secondary peer model. Validators are tracked as
the primary peer set and secondary peers are tracked as followers. Primary peers
drive consensus progress; secondary peers can establish authenticated transport
connections and follow replicated data without signing consensus messages.

The Docker devnet generates `secondary0` during setup, stores its identity in the
`data_secondary0` volume, and lists its public key in
`/shared/peers.json` under `secondary_participants`. Validators load that file on
startup and call Commonware's `Manager::track` with validators as `primary` and
`secondary0` as `secondary`.

To run the built-in secondary peer:

```bash
just devnet

# The secondary peer is exposed on localhost:30500
docker compose -f docker/compose/devnet.yaml ps secondary-node0
```

To join from another process, use an identity that is already listed in
`peers.json` under `secondary_participants`, copy the matching `validator.key`
into the peer's data directory, and start:

```bash
kora secondary \
  --data-dir /path/to/secondary0 \
  --peers /path/to/peers.json \
  --chain-id 1337
```

For a local Docker devnet, `node0:30303` is the bootstrap address inside the
Compose network. From outside Docker, rewrite the bootstrapper address for
`node0` in `peers.json` to the published endpoint `localhost:30400`, and keep
the secondary peer's public key in `peers.json` before starting or restarting
validators.

### Grafana Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `GF_SECURITY_ADMIN_USER` | admin | Grafana admin username |
| `GF_SECURITY_ADMIN_PASSWORD` | admin | Grafana admin password |
| `GF_AUTH_ANONYMOUS_ENABLED` | true | Allow anonymous access |
| `GF_AUTH_ANONYMOUS_ORG_ROLE` | Viewer | Anonymous user role |

## Directory Structure

```
docker/
├── Dockerfile              # Multi-stage build with cargo-chef
├── docker-bake.hcl         # Buildx configuration
├── Justfile                # Command runner
├── README.md               # This file
├── .dockerignore           # Docker build exclusions
├── compose/
│   └── devnet.yaml         # Docker Compose configuration
├── config/
│   └── prometheus.yml      # Prometheus scrape config
├── scripts/
│   ├── devnet-run.sh       # Devnet startup script
│   ├── devnet-stats.sh     # Live monitoring dashboard
│   ├── entrypoint.sh       # Container entrypoint
│   └── healthcheck.sh      # Health check script
└── grafana/
    ├── provisioning/       # Auto-configure datasources
    └── dashboards/         # Pre-built dashboards
```

## Development

Build the image locally:

```bash
cd docker
just build
```

Build without cache:

```bash
just build-fresh
```

Validate compose file:

```bash
just validate
```

Lint Dockerfile:

```bash
just lint
```

Re-run DKG (keeps other state):

```bash
just redo-dkg
```

Access a running container:

```bash
just exec validator-node0
```

View logs for a specific node:

```bash
just logs-node validator-node0
```

## Troubleshooting

**DKG doesn't complete:**
- Check logs: `just logs-dkg`
- Ensure all 4 DKG nodes can reach each other
- Increase timeout if network is slow

**Validators crash on startup:**
- Verify DKG completed: check for `share.key` in data volumes
- Check logs: `just logs`
- Check specific node: `just logs-node validator-node0`

**Port conflicts:**
- Check if ports 30400-30403, 30500, 8545-8548, 9000-9003, 9090, or 3000 are in use
- Stop conflicting services or modify port mappings in `compose/devnet.yaml`

**Container won't start:**
- Check Docker daemon is running
- Ensure sufficient disk space and memory
- Try a fresh build: `just build-fresh`

**Metrics not appearing in Grafana:**
- Verify Prometheus is scraping: http://localhost:9090/targets
- Check validator metrics endpoints are accessible

**Full reset:**
```bash
just reset
just devnet
```

**Restart only validators (preserves DKG state):**
```bash
just restart-validators
```
