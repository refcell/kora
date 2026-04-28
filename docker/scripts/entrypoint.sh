#!/bin/bash
set -euo pipefail

VALIDATOR_INDEX=${VALIDATOR_INDEX:-0}
IS_BOOTSTRAP=${IS_BOOTSTRAP:-false}
BOOTSTRAP_PEERS=${BOOTSTRAP_PEERS:-""}
CHAIN_ID=${CHAIN_ID:-1337}
DATA_DIR=${DATA_DIR:-/data}
SHARED_DIR=${SHARED_DIR:-/shared}

MODE="${1:-validator}"
shift || true

log() { echo "[entrypoint] $*"; }
error() { echo "[entrypoint] ERROR: $*" >&2; exit 1; }

case "$MODE" in
    setup)
        log "Running setup mode..."
        exec /usr/local/bin/keygen setup "$@"
        ;;
        
    dkg)
        log "Running DKG ceremony mode..."
        
        [[ -f "${SHARED_DIR}/peers.json" ]] || error "peers.json not found"
        [[ -f "${DATA_DIR}/validator.key" ]] || error "validator.key not found"
        
        if [[ -f "${DATA_DIR}/share.key" && -f "${DATA_DIR}/output.json" ]]; then
            log "DKG already completed (share.key exists)"
            exit 0
        fi
        
        if [[ "$IS_BOOTSTRAP" != "true" && -n "$BOOTSTRAP_PEERS" ]]; then
            BOOTSTRAP_HOST=$(echo "$BOOTSTRAP_PEERS" | cut -d: -f1)
            BOOTSTRAP_PORT=$(echo "$BOOTSTRAP_PEERS" | cut -d: -f2)
            
            log "Waiting for bootstrap peer ${BOOTSTRAP_HOST}:${BOOTSTRAP_PORT}..."
            timeout=120
            while ! nc -z "$BOOTSTRAP_HOST" "$BOOTSTRAP_PORT" 2>/dev/null; do
                timeout=$((timeout - 1))
                [[ $timeout -le 0 ]] && error "Timeout waiting for bootstrap peer"
                sleep 1
            done
            log "Bootstrap peer reachable"
        fi
        
        exec /usr/local/bin/kora dkg \
            --data-dir "$DATA_DIR" \
            --peers "${SHARED_DIR}/peers.json" \
            --chain-id "$CHAIN_ID" \
            "$@"
        ;;
        
    validator)
        log "Running validator mode..."
        
        [[ -f "${SHARED_DIR}/genesis.json" ]] || error "genesis.json not found"
        [[ -f "${DATA_DIR}/validator.key" ]] || error "validator.key not found"
        [[ -f "${DATA_DIR}/share.key" ]] || error "share.key not found (run DKG first)"
        [[ -f "${DATA_DIR}/output.json" ]] || error "output.json not found (run DKG first)"
        
        cp "${SHARED_DIR}/genesis.json" "${DATA_DIR}/" 2>/dev/null || true
        touch "${DATA_DIR}/.ready"
        
        if [[ "$IS_BOOTSTRAP" != "true" && -n "$BOOTSTRAP_PEERS" ]]; then
            BOOTSTRAP_HOST=$(echo "$BOOTSTRAP_PEERS" | cut -d: -f1)
            BOOTSTRAP_PORT=$(echo "$BOOTSTRAP_PEERS" | cut -d: -f2)
            
            log "Waiting for bootstrap peer ${BOOTSTRAP_HOST}:${BOOTSTRAP_PORT}..."
            timeout=120
            while ! nc -z "$BOOTSTRAP_HOST" "$BOOTSTRAP_PORT" 2>/dev/null; do
                timeout=$((timeout - 1))
                [[ $timeout -le 0 ]] && error "Timeout waiting for bootstrap peer"
                sleep 1
            done
        fi
        
        exec /usr/local/bin/kora validator \
            --data-dir "$DATA_DIR" \
            --peers "${SHARED_DIR}/peers.json" \
            --chain-id "$CHAIN_ID" \
            "$@"
        ;;

    secondary)
        log "Running secondary peer mode..."

        [[ -f "${SHARED_DIR}/peers.json" ]] || error "peers.json not found"
        [[ -f "${DATA_DIR}/validator.key" ]] || error "validator.key not found"

        touch "${DATA_DIR}/.ready"

        if [[ "$IS_BOOTSTRAP" != "true" && -n "$BOOTSTRAP_PEERS" ]]; then
            BOOTSTRAP_HOST=$(echo "$BOOTSTRAP_PEERS" | cut -d: -f1)
            BOOTSTRAP_PORT=$(echo "$BOOTSTRAP_PEERS" | cut -d: -f2)

            log "Waiting for bootstrap peer ${BOOTSTRAP_HOST}:${BOOTSTRAP_PORT}..."
            timeout=120
            while ! nc -z "$BOOTSTRAP_HOST" "$BOOTSTRAP_PORT" 2>/dev/null; do
                timeout=$((timeout - 1))
                [[ $timeout -le 0 ]] && error "Timeout waiting for bootstrap peer"
                sleep 1
            done
        fi

        exec /usr/local/bin/kora secondary \
            --data-dir "$DATA_DIR" \
            --peers "${SHARED_DIR}/peers.json" \
            --chain-id "$CHAIN_ID" \
            "$@"
        ;;
        
    *)
        exec "$MODE" "$@"
        ;;
esac
