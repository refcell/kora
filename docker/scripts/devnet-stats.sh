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
    echo -e "│ ${BOLD}Node${NC}  │ ${BOLD}RPC${NC}      │ ${BOLD}Uptime${NC}     │ ${BOLD}View${NC}     │ ${BOLD}Finalized${NC}  │ ${BOLD}Nullified${NC}  │ ${BOLD}Proposed${NC}   │ ${BOLD}Throughput${NC} │ ${BOLD}Leader${NC} │"
    echo -e "├───────┼──────────┼────────────┼──────────┼────────────┼────────────┼────────────┼────────────┼────────┤"
    
    local healthy_count=0
    local max_uptime=0
    local total_finalized=0
    local max_view=0
    local max_throughput=0
    
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
            parsed=$(echo "$status" | jq -r '[.uptime_secs // 0, .current_view // 0, .finalized_count // 0, .nullified_count // 0, .proposed_count // 0, .is_leader // false] | @tsv' 2>/dev/null)
            
            if [[ -n "$parsed" ]]; then
                read -r uptime view finalized nullified proposed leader <<< "$parsed"
                
                uptime="${uptime:-0}"
                view="${view:-0}"
                finalized="${finalized:-0}"
                nullified="${nullified:-0}"
                proposed="${proposed:-0}"
                
                [[ $uptime -gt $max_uptime ]] && max_uptime=$uptime
                [[ $view -gt $max_view ]] && max_view=$view
                total_finalized=$finalized
                ((healthy_count++))
                
                local uptime_str=$(format_uptime "$uptime")
                local leader_str="-"
                [[ "$leader" == "true" ]] && leader_str="${MAGENTA}★${NC}"
                
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
                
                printf "│ ${CYAN}%-5s${NC} │ ${GREEN}online${NC}   │ %-10s │ %-8s │ %-10s │ %-10s │ %-10s │ %-10s │   %b    │\n" \
                    "$i" "$uptime_str" "$view" "$finalized" "$nullified" "$proposed" "$throughput_str" "$leader_str"
            else
                printf "│ ${CYAN}%-5s${NC} │ ${RED}offline${NC}  │ -          │ -        │ -          │ -          │ -          │ -          │   -    │\n" "$i"
            fi
        else
            printf "│ ${CYAN}%-5s${NC} │ ${RED}offline${NC}  │ -          │ -        │ -          │ -          │ -          │ -          │   -    │\n" "$i"
        fi
        ((i++))
    done <<< "$all_status"
    
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
    
    echo -e "  ${DIM}Healthy:${NC} ${health_color}${healthy_count}/4${NC}  │  ${DIM}Threshold:${NC} $threshold  │  ${DIM}View:${NC} ${CYAN}$max_view${NC}  │  ${DIM}Finalized:${NC} ${GREEN}$total_finalized${NC}  │  ${DIM}Throughput:${NC} ${CYAN}$throughput_str${NC}  │  ${DIM}Uptime:${NC} $uptime_str"
    
    # Endpoints
    echo ""
    echo -e "${BOLD}${CYAN}Endpoints${NC}"
    echo -e "  ${DIM}P2P:${NC} 30400-30403    ${DIM}RPC:${NC} 8545-8548    ${DIM}Metrics:${NC} 9000-9003"
    
    # Clear extra lines
    for _ in {1..3}; do
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
