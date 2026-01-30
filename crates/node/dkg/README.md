# `kora-dkg`

[![CI](https://github.com/refcell/kora/actions/workflows/ci.yml/badge.svg)](https://github.com/refcell/kora/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

Interactive Distributed Key Generation (DKG) for Kora threshold cryptography.

Implements the Joint-Feldman DKG protocol using [commonware-cryptography](https://docs.rs/commonware-cryptography) BLS12-381 primitives. Each participant contributes to the key generation process, and no single party learns the master secret.

## Key Types

- `DkgCeremony` - Orchestrates the full interactive DKG ceremony with crash recovery
- `DkgParticipant` - State machine for a participant (both dealer and player)
- `DkgConfig` - Configuration for DKG parameters
- `DkgNetwork` - Basic TCP networking (development use only)
- `DkgTransport` - Production-ready authenticated transport using commonware-p2p
- `DkgOutput` - Output containing generated keys and shares
- `DkgPhase` - Ceremony phase tracking for crash recovery
- `PersistedDkgState` - Persistent state for resuming interrupted ceremonies
- `CeremonySession` - Session metadata for anti-replay protection

## Security Features

- **Session Binding**: All messages include a ceremony ID derived from chain ID, participants, and timestamp
- **Anti-Replay Protection**: Message deduplication via SHA256 hashes
- **Membership Validation**: Only accept messages from known participants
- **Sender Verification**: Verify that claimed sender matches actual sender identity
- **First-Seen-Wins**: Prevent overwriting of already-received dealer messages
- **Bounds Checking**: Limit message counts to prevent resource exhaustion
- **Authenticated Transport**: Production transport uses commonware-p2p with encrypted channels

## Protocol Flow

1. **Setup**: Each node loads its Ed25519 identity key and participant list
2. **Dealer Phase**: Each node generates commitment polynomials and shares for all players
3. **Player Phase**: Each node verifies received shares and sends acknowledgements
4. **Finalization**: Dealers create signed logs, leader coordinates transcript agreement
5. **Output**: Each node computes its BLS12-381 share and the group public key

## Crash Recovery

The ceremony automatically persists state to `{data_dir}/dkg_state.json` after each phase transition. On restart:

- If state exists and session matches, the ceremony resumes from the last completed phase
- If state exists but session doesn't match, state is cleared and ceremony starts fresh
- Use `--force-restart` flag to ignore persisted state

## Condition-Based Phase Transitions

Rather than fixed timeouts, the ceremony advances based on quorum conditions:

- Phase 2 waits until quorum dealer messages received AND acks sent
- Phase 4 waits until quorum dealer logs collected
- Each phase has a maximum timeout (120s by default) with proper error reporting

## Usage

### Interactive DKG Ceremony

```bash
kora dkg --peers /path/to/peers.json --data-dir /path/to/data
```

### Force Restart (ignore persisted state)

```bash
kora dkg --peers /path/to/peers.json --data-dir /path/to/data --force-restart
```

## License

[MIT License](https://opensource.org/licenses/MIT)
