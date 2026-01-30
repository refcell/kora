# DKG Migration Plan: Trusted Dealer → Interactive DKG

## Overview

Migrate the devnet from a **trusted dealer DKG** (insecure, single party knows all secrets) to a **production interactive DKG** (Joint-Feldman style, no single party learns the master secret).

## Current State

| Component | Status |
|-----------|--------|
| `just devnet` | Runs trusted dealer via `keygen dkg-deal` |
| `kora-dkg` crate | Has structure but `ceremony.rs::run()` just calls `dkg::deal()` locally |
| Docker `dkg-node0..3` | Exist under `distributed-dkg` profile but not wired up |

## Target State

| Component | Status |
|-----------|--------|
| `just trusted-devnet` | Runs trusted dealer (for fast local dev) |
| `just devnet` | Runs interactive DKG ceremony |
| `kora-dkg` crate | Full interactive DKG using commonware's Joint-Feldman primitives |

---

## Phase 1: Rename Current Devnet (Quick Win)

### Changes

1. **Root Justfile**: Rename `devnet` → `trusted-devnet`, add new `devnet`
2. **docker/Justfile**: Add `trusted-devnet` recipe
3. **Docker Compose**: Add `trusted-dkg` profile for trusted dealer flow

### Files to Modify

- `/Justfile`
- `/docker/Justfile`
- `/docker/compose/devnet.yaml`

---

## Phase 2: Implement Interactive DKG Ceremony

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Interactive DKG Protocol                          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   Node0 (Leader)        Node1              Node2              Node3     │
│   ┌──────────┐         ┌──────────┐       ┌──────────┐       ┌──────────┐
│   │ Dealer   │         │ Dealer   │       │ Dealer   │       │ Dealer   │
│   │ Player   │         │ Player   │       │ Player   │       │ Player   │
│   └──────────┘         └──────────┘       └──────────┘       └──────────┘
│                                                                          │
│   1. Each node creates DealerPubMsg (commitment) + DealerPrivMsgs       │
│   2. Broadcast DealerPubMsg to all                                       │
│   3. Send DealerPrivMsg (encrypted share) to each specific player       │
│   4. Players verify shares, send PlayerAck back to dealers              │
│   5. Dealers collect acks, create SignedDealerLog                       │
│   6. Leader collects all SignedDealerLogs, broadcasts agreed transcript │
│   7. All nodes finalize using same transcript → same group key          │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Key Commonware Types to Use

```rust
use commonware_cryptography::bls12381::dkg::{
    Info,           // Round configuration
    Dealer,         // Generates shares for others
    Player,         // Receives and verifies shares
    SignedDealerLog,// Finalized dealer contribution
    observe,        // For computing public output from logs
};
use commonware_cryptography::bls12381::primitives::{
    variant::MinSig,
    sharing::Mode,
};
use commonware_utils::N3f1; // n=3f+1 quorum rules
```

### Protocol Flow

1. **Build participants set** from `setup.json` files
2. **Create round info** with `dkg::Info::new::<N3f1>(...)`
3. **Each node starts as Dealer+Player**:
   - `Dealer::start()` → `DealerPubMsg` + `DealerPrivMsg` per player
4. **Network phase**:
   - Broadcast `DealerPubMsg` to all
   - Send `DealerPrivMsg` privately to each player
5. **Players verify and ack**:
   - `Player::dealer_message()` → `PlayerAck`
   - Send ack back to dealer
6. **Dealers finalize**:
   - `Dealer::receive_player_ack()` for each ack
   - `Dealer::finalize()` → `SignedDealerLog`
   - Send log to leader (node0)
7. **Leader broadcasts agreed transcript**:
   - Verify all logs with `SignedDealerLog::check()`
   - Broadcast collected logs to all nodes
8. **All nodes finalize**:
   - `Player::finalize()` with agreed logs
   - Persist `output.json` and `share.key`

### Files to Create/Modify

| File | Action |
|------|--------|
| `crates/node/dkg/src/ceremony.rs` | Rewrite with interactive protocol |
| `crates/node/dkg/src/protocol.rs` | New: Protocol state machine |
| `crates/node/dkg/src/network.rs` | New: P2P messaging layer |
| `crates/node/dkg/src/message.rs` | Replace with commonware types |
| `bin/kora/src/cli.rs` | Update DKG command |

---

## Phase 3: Docker Orchestration

### Container Flow

```
init-config (setup only)
    │
    ▼
┌─────────────────────────────────────────────────┐
│  DKG Phase (one-shot jobs)                       │
│                                                  │
│  dkg-node0 ──┐                                  │
│  dkg-node1 ──┼── Interactive ceremony ──► exit 0 │
│  dkg-node2 ──┤                                  │
│  dkg-node3 ──┘                                  │
└─────────────────────────────────────────────────┘
    │
    ▼ (all DKG jobs completed successfully)
    │
┌─────────────────────────────────────────────────┐
│  Validator Phase (long-running)                  │
│                                                  │
│  validator-node0                                 │
│  validator-node1                                 │
│  validator-node2                                 │
│  validator-node3                                 │
└─────────────────────────────────────────────────┘
```

### Compose Changes

1. **Split init-config**:
   - `init-config`: Setup only (always runs)
   - `init-config-trusted`: Setup + `dkg-deal` (trusted-dkg profile)

2. **DKG nodes as jobs**:
   - Remove healthcheck (use `service_completed_successfully`)
   - Add proper startup ordering with retries in code

3. **Validators depend on DKG completion**:
   - `depends_on: dkg-nodeX: condition: service_completed_successfully`

---

## Phase 4: Testing & Validation

### Test Cases

1. **Happy path**: All 4 nodes complete DKG, validators start
2. **Network delay**: Nodes start with staggered timing
3. **Restart resilience**: Kill and restart a DKG node mid-ceremony
4. **Output verification**: All nodes derive same group public key

### Validation Commands

```bash
# Run interactive devnet
just devnet

# Verify all nodes have same group key
for i in 0 1 2 3; do
  docker run --rm -v kora-devnet_data_node${i}:/data alpine \
    cat /data/output.json | jq -r .group_public_key
done | sort -u | wc -l
# Should output: 1

# Run trusted devnet for quick iteration
just trusted-devnet
```

---

## Implementation Order

1. **[Phase 1]** Rename commands ✅ DONE
   - Added `just trusted-devnet` command
   - `just devnet` now runs interactive DKG flow
   - Updated Docker compose with `init-setup` service
   - Changed DKG profile to `interactive-dkg`

2. **[Phase 2]** Implement interactive DKG ✅ DONE
   - Created `protocol.rs` with `DkgParticipant` state machine
   - Created `network.rs` with TCP-based messaging
   - Rewrote `ceremony.rs` to use interactive protocol
   - Uses commonware's Joint-Feldman DKG primitives

3. **[Phase 3]** Production hardening ✅ DONE
   - **Session binding**: Added `CeremonySession` with ceremony ID derived from chain + participants + timestamp
   - **Anti-replay**: Message deduplication via SHA256 hashes in `seen_messages` set
   - **Membership validation**: Verify all senders are in participants list
   - **Sender verification**: Check `from == claimed dealer/player` in all message handlers
   - **First-seen-wins**: Reject duplicate dealer messages (no overwrites)
   - **Bounds checking**: Limit message counts to `n` participants
   - **Condition-based phases**: Replaced fixed 60s timeouts with quorum-based transitions
   - **Crash recovery**: Added `PersistedDkgState` for resuming interrupted ceremonies
   - **Authenticated transport**: Created `DkgTransport` using commonware-p2p discovery

4. **[Phase 4]** Docker orchestration - TODO
   - Test the full interactive DKG flow in Docker
   - Switch from TCP network to authenticated transport in ceremony
   - Verify health checks and completion detection

5. **[Phase 5]** Testing - TODO
   - Integration tests for the DKG ceremony in Docker
   - End-to-end tests with multiple nodes
   - Failure scenario tests (node crashes, network partitions)

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Share index mismatch | Always persist `share.index` from commonware, don't assume `VALIDATOR_INDEX` |
| Transcript disagreement | Leader is single source of truth for agreed logs |
| Startup deadlocks | Implement connection retries + exponential backoff in code |
| Partial DKG failure | Set generous timeout; condition-based phase transitions |
| Replay attacks | Ceremony ID + message deduplication |
| Impersonation | Sender verification in all message handlers |
| Resource exhaustion | Bounds checking on all message maps |

---

## Future Enhancements

- **Decentralized agreement**: Replace leader with threshold agreement on logs
- **Resharing support**: Use `dkg::Info` with `previous_output` for key rotation
- **Chain-integrated DKG**: Run ceremony as part of chain bootstrapping
- **Integrate authenticated transport**: Switch ceremony.rs to use DkgTransport instead of DkgNetwork
