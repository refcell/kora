#!/usr/bin/env bash
set -e

# Parse arguments
INTERACTIVE_DKG=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --interactive-dkg)
            INTERACTIVE_DKG=true
            shift
            ;;
        *)
            shift
            ;;
    esac
done

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

# Icons
CHECK="✓"
CROSS="✗"
ARROW="→"
SPIN="◐◓◑◒"

spin_idx=0
spinner() {
    printf "\r  ${CYAN}${SPIN:spin_idx:1}${NC} %s" "$1"
    spin_idx=$(( (spin_idx + 1) % 4 ))
}

clear_line() {
    printf "\r                                                              \r"
}

# Run a command with a spinner, suppressing output
run_with_spinner() {
    local msg=$1
    shift
    local logfile=$(mktemp)
    
    # Start command in background
    "$@" > "$logfile" 2>&1 &
    local pid=$!
    
    # Spin while waiting
    while kill -0 "$pid" 2>/dev/null; do
        spinner "$msg"
        sleep 0.15
    done
    
    # Get exit code
    wait "$pid"
    local exit_code=$?
    
    clear_line
    
    if [[ $exit_code -ne 0 ]]; then
        cat "$logfile"
        rm -f "$logfile"
        return $exit_code
    fi
    
    rm -f "$logfile"
    return 0
}

print_header() {
    echo ""
    echo -e "${BOLD}${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
    if [[ "$INTERACTIVE_DKG" == "true" ]]; then
        echo -e "${BOLD}${BLUE}║${NC}        ${BOLD}KORA DEVNET${NC} ${GREEN}(Interactive DKG)${NC}                 ${BOLD}${BLUE}║${NC}"
    else
        echo -e "${BOLD}${BLUE}║${NC}        ${BOLD}KORA DEVNET${NC} ${YELLOW}(Trusted Dealer)${NC}                  ${BOLD}${BLUE}║${NC}"
    fi
    echo -e "${BOLD}${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "  ${DIM}Chain ID:${NC} ${CHAIN_ID:-1337}  ${DIM}│${NC}  ${DIM}Validators:${NC} 4  ${DIM}│${NC}  ${DIM}Threshold:${NC} 3"
    echo ""
}

print_phase() {
    local phase=$1
    local desc=$2
    echo -e "${BOLD}${CYAN}[$phase]${NC} ${BOLD}$desc${NC}"
}

print_success() {
    echo -e "  ${GREEN}${CHECK}${NC} $1"
}

print_skip() {
    echo -e "  ${YELLOW}${ARROW}${NC} $1 ${DIM}(cached)${NC}"
}

print_error() {
    echo -e "  ${RED}${CROSS}${NC} $1"
}

print_endpoints() {
    echo ""
    echo -e "${BOLD}Endpoints${NC}"
    echo -e "  ${DIM}P2P:${NC}        localhost:30400-30403"
    echo -e "  ${DIM}Prometheus:${NC} http://localhost:9090"
    echo -e "  ${DIM}Grafana:${NC}    http://localhost:3000"
    echo ""
    echo -e "${DIM}Run 'just devnet-stats' for live monitoring${NC}"
    echo ""
}

cd "$(dirname "$0")/.."

print_header

# Phase 0: Build
print_phase "0/3" "Building Docker image"
if run_with_spinner "Building kora:local image..." docker buildx bake --allow=fs.read=.. -f docker-bake.hcl kora-local; then
    print_success "Image built successfully"
else
    print_error "Build failed"
    exit 1
fi

# Check existing state
CONFIG_EXISTS=false
SHARES_EXIST=false

docker volume inspect kora-devnet_shared_config >/dev/null 2>&1 && \
    docker run --rm -v kora-devnet_shared_config:/shared alpine test -f /shared/peers.json 2>/dev/null && \
    CONFIG_EXISTS=true

docker volume inspect kora-devnet_data_node0 >/dev/null 2>&1 && \
    docker run --rm -v kora-devnet_data_node0:/data alpine test -f /data/share.key 2>/dev/null && \
    SHARES_EXIST=true

echo ""

# Phase 1: Configuration
print_phase "1/3" "Configuration"
if [[ "$CONFIG_EXISTS" != "true" ]]; then
    if [[ "$INTERACTIVE_DKG" == "true" ]]; then
        # Interactive DKG: only run setup (no dkg-deal)
        if run_with_spinner "Generating peer configuration..." docker compose -f compose/devnet.yaml run --rm init-setup; then
            print_success "Generated peer configuration"
        else
            print_error "Configuration failed"
            exit 1
        fi
    else
        # Trusted dealer: run setup + dkg-deal
        if run_with_spinner "Generating peer configuration..." docker compose -f compose/devnet.yaml run --rm init-config; then
            print_success "Generated peer configuration"
        else
            print_error "Configuration failed"
            exit 1
        fi
    fi
else
    print_skip "Peer configuration exists"
fi

if [[ "$INTERACTIVE_DKG" == "true" ]]; then
    if [[ "$SHARES_EXIST" != "true" ]]; then
        echo ""
        print_phase "1.5/3" "Interactive DKG Ceremony"
        
        # Start DKG nodes
        run_with_spinner "Starting DKG nodes..." docker compose -f compose/devnet.yaml --profile interactive-dkg up -d \
            dkg-node0 dkg-node1 dkg-node2 dkg-node3
        
        # Wait for DKG completion
        start_time=$(date +%s)
        timeout=300  # 5 minutes for DKG
        
        while true; do
            # Check if all DKG containers have exited successfully (use -a to include stopped containers)
            EXITED=$(docker compose -f compose/devnet.yaml ps -a --format json 2>/dev/null | \
                jq -r 'select(.Service | startswith("dkg-")) | select(.State == "exited") | select(.ExitCode == 0) | .Service' 2>/dev/null | wc -l | tr -d ' ')
            
            FAILED=$(docker compose -f compose/devnet.yaml ps -a --format json 2>/dev/null | \
                jq -r 'select(.Service | startswith("dkg-")) | select(.State == "exited") | select(.ExitCode != 0) | .Service' 2>/dev/null | wc -l | tr -d ' ')
            
            elapsed=$(($(date +%s) - start_time))
            
            if [[ "$FAILED" -gt 0 ]]; then
                clear_line
                print_error "DKG ceremony failed"
                echo ""
                echo -e "${RED}DKG node logs:${NC}"
                docker compose -f compose/devnet.yaml logs dkg-node0 dkg-node1 dkg-node2 dkg-node3 --tail=50
                exit 1
            fi
            
            if [[ "$EXITED" -ge 4 ]]; then
                clear_line
                print_success "Interactive DKG ceremony completed"
                break
            fi
            
            if [[ "$elapsed" -ge "$timeout" ]]; then
                clear_line
                print_error "Timeout waiting for DKG ceremony"
                exit 1
            fi
            
            spinner "Running DKG ceremony... (${elapsed}s)"
            sleep 0.15
        done
        
        # Stop DKG containers (they should already be stopped)
        docker compose -f compose/devnet.yaml --profile interactive-dkg stop dkg-node0 dkg-node1 dkg-node2 dkg-node3 2>/dev/null || true
    else
        print_skip "DKG shares exist"
    fi
else
    if [[ "$SHARES_EXIST" != "true" ]]; then
        print_success "Threshold shares generated (trusted dealer)"
    else
        print_skip "Threshold shares exist"
    fi
fi

echo ""

# Phase 2: Validators
print_phase "2/3" "Starting validators"

run_with_spinner "Launching validator containers..." docker compose -f compose/devnet.yaml ${COMPOSE_PROFILES:+--profile observability} up -d \
    validator-node0 validator-node1 validator-node2 validator-node3 \
    ${COMPOSE_PROFILES:+prometheus grafana}

# Wait for validators with spinner
start_time=$(date +%s)
timeout=120

while true; do
    HEALTHY=$(docker compose -f compose/devnet.yaml ps --format json 2>/dev/null | \
        jq -r 'select(.Service | startswith("validator-")) | select(.Health == "healthy") | .Service' 2>/dev/null | wc -l | tr -d ' ')
    
    elapsed=$(($(date +%s) - start_time))
    
    if [[ "$HEALTHY" -ge 4 ]]; then
        clear_line
        print_success "All 4 validators healthy"
        break
    fi
    
    if [[ "$elapsed" -ge "$timeout" ]]; then
        clear_line
        print_error "Timeout waiting for validators"
        exit 1
    fi
    
    spinner "Waiting for validators... (${HEALTHY}/4 healthy, ${elapsed}s)"
    sleep 0.15
done

echo ""

# Phase 3: Ready
print_phase "3/3" "Devnet ready"

echo ""
echo -e "  ${GREEN}┌────────────┬────────────┬─────────┐${NC}"
echo -e "  ${GREEN}│${NC} ${BOLD}Node${NC}       ${GREEN}│${NC} ${BOLD}Status${NC}     ${GREEN}│${NC} ${BOLD}Port${NC}    ${GREEN}│${NC}"
echo -e "  ${GREEN}├────────────┼────────────┼─────────┤${NC}"

for i in 0 1 2 3; do
    status=$(docker compose -f compose/devnet.yaml ps --format json 2>/dev/null | \
        jq -r "select(.Service == \"validator-node$i\") | .Health" 2>/dev/null || echo "unknown")
    
    if [[ "$status" == "healthy" ]]; then
        status_str="${GREEN}healthy${NC}    "
    else
        status_str="${YELLOW}${status}${NC}"
    fi
    
    printf "  ${GREEN}│${NC} node%-6s ${GREEN}│${NC} %b ${GREEN}│${NC} 3040%-3s ${GREEN}│${NC}\n" "$i" "$status_str" "$i"
done

echo -e "  ${GREEN}└────────────┴────────────┴─────────┘${NC}"

print_endpoints
