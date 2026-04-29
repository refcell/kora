#!/bin/bash
set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

REFRESH_INTERVAL=${1:-0.3}
CHAIN_ID="${CHAIN_ID:-1337}"
RPC_PORTS=(8545 8546 8547 8548)
FOLLOWER_SERVICE="secondary-node0"
FOLLOWER_P2P_PORT=30500

cleanup() {
    tput cnorm
    echo ""
    exit 0
}
trap cleanup INT TERM

format_uptime() {
    local s=$1
    if [[ $s -ge 86400 ]]; then printf "%dd%dh" $((s/86400)) $((s%86400/3600))
    elif [[ $s -ge 3600 ]]; then printf "%dh%dm" $((s/3600)) $((s%3600/60))
    elif [[ $s -ge 60 ]]; then printf "%dm%ds" $((s/60)) $((s%60))
    else printf "%ds" $s; fi
}

# Fetch all node statuses in parallel
fetch_all_statuses() {
    local tmpdir=$(mktemp -d)
    
    # Launch parallel fetches using JSON-RPC POST to get block number (indicates node is alive)
    for i in 0 1 2 3; do
        (curl -s --max-time 0.2 -X POST -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"kora_nodeStatus","params":[],"id":1}' \
            "http://localhost:${RPC_PORTS[$i]}" 2>/dev/null | \
            jq -c '.result // {}' > "$tmpdir/$i" 2>/dev/null || echo "{}" > "$tmpdir/$i") &
    done
    wait
    
    # Read results
    for i in 0 1 2 3; do
        cat "$tmpdir/$i"
        echo  # newline separator
    done
    
    rm -rf "$tmpdir"
}

fetch_follower_info() {
    docker compose -f compose/devnet.yaml ps --format json 2>/dev/null | \
        jq -r "select(.Service == \"$FOLLOWER_SERVICE\") | [
            .Health // .State // \"unknown\",
            .State // \"unknown\",
            (.RunningFor // \"-\"),
            ([.Publishers[]? | select(.TargetPort == 30303 and .PublishedPort != 0) | .PublishedPort] | unique | join(\",\")),
            .Name // \"$FOLLOWER_SERVICE\"
        ] | @tsv" 2>/dev/null || true
}

render() {
    tput cup 0 0
    local now=$(date "+%H:%M:%S")
    
    echo -e "${BOLD}${BLUE}╔══════════════════════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BOLD}${BLUE}║${NC}                              ${BOLD}KORA DEVNET MONITOR${NC}                                        ${BOLD}${BLUE}║${NC}"
    echo -e "${BOLD}${BLUE}╚══════════════════════════════════════════════════════════════════════════════════════════╝${NC}"
    echo -e "  ${DIM}$now${NC}  │  ${DIM}Chain:${NC} ${CYAN}$CHAIN_ID${NC}  │  ${DIM}Refresh:${NC} ${REFRESH_INTERVAL}s  │  ${DIM}Ctrl+C to exit${NC}"
    echo ""
    
    echo -e "${BOLD}${CYAN}Node Status${NC}"
    echo -e "┌───────┬──────────┬────────────┬──────────┬────────────┬────────────┬────────────┬────────────┬────────┐"
    echo -e "│ ${BOLD}Node${NC}  │ ${BOLD}Status${NC}   │ ${BOLD}Uptime${NC}     │ ${BOLD}View${NC}     │ ${BOLD}Finalized${NC}  │ ${BOLD}Nullified${NC}  │ ${BOLD}Proposed${NC}   │ ${BOLD}Throughput${NC} │ ${BOLD}Leader${NC} │"
    echo -e "├───────┼──────────┼────────────┼──────────┼────────────┼────────────┼────────────┼────────────┼────────┤"
    
    local rpc_count=0
    local healthy_count=0
    local stalled_count=0
    local max_uptime=0
    local total_finalized=0
    local max_view=0
    local max_throughput=0
    local follower_status="offline"
    local follower_color=$RED
    local follower_state="-"
    local follower_uptime="-"
    local follower_p2p="$FOLLOWER_P2P_PORT"
    local follower_container="$FOLLOWER_SERVICE"
    
    # Fetch all statuses in parallel
    local all_status
    all_status=$(fetch_all_statuses)
    
    local i=0
    while IFS= read -r status; do
        # Skip empty lines (separators between node outputs)
        [[ -z "$status" ]] && continue
        
        if [[ "$status" != "{}" ]]; then
            # Parse with single jq call
            local parsed
            parsed=$(echo "$status" | jq -r '[.validatorIndex // .validator_index // empty, .uptimeSecs // .uptime_secs // 0, .currentView // .current_view // 0, .finalizedCount // .finalized_count // 0, .nullifiedCount // .nullified_count // 0, .proposedCount // .proposed_count // 0, .isLeader // .is_leader // false] | @tsv' 2>/dev/null)
            
            if [[ -n "$parsed" ]]; then
                read -r validator_index uptime view finalized nullified proposed leader <<< "$parsed"
                
                validator_index="${validator_index:-$i}"
                uptime="${uptime:-0}"
                view="${view:-0}"
                finalized="${finalized:-0}"
                nullified="${nullified:-0}"
                proposed="${proposed:-0}"
                
                [[ $uptime -gt $max_uptime ]] && max_uptime=$uptime
                [[ $view -gt $max_view ]] && max_view=$view
                total_finalized=$finalized
                ((++rpc_count))
                
                local uptime_str=$(format_uptime "$uptime")
                local leader_str="-"
                [[ "$leader" == "true" ]] && leader_str="${MAGENTA}★${NC}"
                local rpc_status="${GREEN}online${NC} "
                if [[ $view -eq 0 && $finalized -eq 0 && $proposed -eq 0 && $uptime -gt 10 ]]; then
                    rpc_status="${YELLOW}stalled${NC}"
                    ((++stalled_count))
                else
                    ((++healthy_count))
                fi
                
                # Calculate throughput (blocks/sec)
                local throughput_str="-"
                if [[ $uptime -gt 0 && $finalized -gt 0 ]]; then
                    # Use awk for floating point division
                    local tps=$(awk "BEGIN {printf \"%.2f\", $finalized / $uptime}")
                    throughput_str="${tps} b/s"
                    # Track max for summary
                    local tps_int=$(awk "BEGIN {printf \"%d\", $finalized * 100 / $uptime}")
                    [[ $tps_int -gt $max_throughput ]] && max_throughput=$tps_int
                fi
                
                printf "│ ${CYAN}%-5s${NC} │ %b │ %-10s │ %-8s │ %-10s │ %-10s │ %-10s │ %-10s │   %b    │\n" \
                    "$validator_index" "$rpc_status" "$uptime_str" "$view" "$finalized" "$nullified" "$proposed" "$throughput_str" "$leader_str"
            else
                printf "│ ${CYAN}%-5s${NC} │ ${RED}offline${NC}  │ -          │ -        │ -          │ -          │ -          │ -          │   -    │\n" "$i"
            fi
        else
            printf "│ ${CYAN}%-5s${NC} │ ${RED}offline${NC}  │ -          │ -        │ -          │ -          │ -          │ -          │   -    │\n" "$i"
        fi
        ((++i))
    done <<< "$all_status"

    local follower_info
    follower_info=$(fetch_follower_info)
    if [[ -n "$follower_info" ]]; then
        local follower_health_value
        IFS=$'\t' read -r follower_health_value follower_state follower_uptime follower_p2p follower_container <<< "$follower_info"
        follower_uptime="${follower_uptime% ago}"
        follower_p2p="${follower_p2p:-$FOLLOWER_P2P_PORT}"

        case "$follower_health_value" in
            healthy)
                follower_status="healthy"
                follower_color=$GREEN
                ;;
            running)
                follower_status="running"
                follower_color=$GREEN
                ;;
            starting)
                follower_status="starting"
                follower_color=$YELLOW
                ;;
            *)
                follower_status="${follower_health_value:-${follower_state:-unknown}}"
                follower_color=$YELLOW
                ;;
        esac
    fi

    local follower_table_uptime="${follower_uptime:0:10}"
    local follower_network="P2P ${follower_p2p:-none}"
    printf "│ ${CYAN}%-5s${NC} │ ${follower_color}%-8s${NC} │ %-10s │ %-8s │ %-10s │ %-10s │ %-10s │ %-10s │   -    │\n" \
        "f0" "$follower_status" "$follower_table_uptime" "follower" "-" "-" "-" "$follower_network"
    
    echo -e "└───────┴──────────┴────────────┴──────────┴────────────┴────────────┴────────────┴────────────┴────────┘"
    
    # Summary
    echo ""
    echo -e "${BOLD}${CYAN}Summary${NC}"
    
    local health_color=$GREEN
    [[ $healthy_count -lt 4 ]] && health_color=$YELLOW
    [[ $healthy_count -lt 3 ]] && health_color=$RED
    
    local threshold="${GREEN}✓ Met${NC}"
    [[ $healthy_count -lt 3 ]] && threshold="${RED}✗ Not met${NC}"
    
    local uptime_str="0s"
    [[ $max_uptime -gt 0 ]] && uptime_str=$(format_uptime "$max_uptime")
    
    # Format throughput from stored integer (x100)
    local throughput_str="0.00 b/s"
    if [[ $max_throughput -gt 0 ]]; then
        throughput_str=$(awk "BEGIN {printf \"%.2f b/s\", $max_throughput / 100}")
    fi
    
    echo -e "  ${DIM}Consensus:${NC} ${health_color}${healthy_count}/4${NC}  │  ${DIM}RPC:${NC} ${GREEN}${rpc_count}/4${NC}  │  ${DIM}Follower:${NC} ${follower_color}${follower_status}${NC}  │  ${DIM}Stalled:${NC} ${YELLOW}${stalled_count}${NC}  │  ${DIM}Threshold:${NC} $threshold  │  ${DIM}View:${NC} ${CYAN}$max_view${NC}  │  ${DIM}Finalized:${NC} ${GREEN}$total_finalized${NC}  │  ${DIM}Throughput:${NC} ${CYAN}$throughput_str${NC}  │  ${DIM}Uptime:${NC} $uptime_str"

    echo ""
    echo -e "${BOLD}${CYAN}Follower Node${NC}"
    echo -e "  ${DIM}Node:${NC} ${CYAN}f0${NC}  │  ${DIM}Role:${NC} secondary  │  ${DIM}Service:${NC} $FOLLOWER_SERVICE  │  ${DIM}Container:${NC} $follower_container"
    echo -e "  ${DIM}Health:${NC} ${follower_color}${follower_status}${NC}  │  ${DIM}State:${NC} $follower_state  │  ${DIM}Uptime:${NC} $follower_uptime  │  ${DIM}P2P:${NC} ${follower_p2p:-none}  │  ${DIM}RPC:${NC} none"
    
    # Endpoints
    echo ""
    echo -e "${BOLD}${CYAN}Endpoints${NC}"
    echo -e "  ${DIM}P2P:${NC} 30400-30403    ${DIM}Follower P2P:${NC} $FOLLOWER_P2P_PORT    ${DIM}RPC:${NC} 8545-8548    ${DIM}Metrics:${NC} 9000-9003"
    
    # Clear extra lines
    for _ in {1..5}; do
        printf "%-90s\n" ""
    done
}

# Main
clear
tput civis

echo -e "${DIM}Connecting to RPC endpoints...${NC}"
sleep 0.2

render

while true; do
    sleep "$REFRESH_INTERVAL"
    render
done
