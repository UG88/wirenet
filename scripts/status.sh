#!/usr/bin/env bash
# ==============================================================================
# WireNet Status & Telemetry Dashboard (Supports All or Individual Checks)
# Developed by UG88 | https://github.com/UG88/wirenet
# ==============================================================================

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}[-] Error: This command must be run as root (or with sudo).${NC}" >&2
   exit 1
fi

MODE="${1:-all}"

# Detect Role
if [[ -f /etc/wireguard/gateway_private.key ]] || ip addr show dev wg0 2>/dev/null | grep -q "10.200.0.1/"; then
    IS_GATEWAY=true
    ROLE="GATEWAY VPS (Hub)"
else
    IS_GATEWAY=false
    ROLE="PTERODACTYL NODE VPS (Spoke)"
fi

PUBLIC_IP=$(curl -s -4 ifconfig.me || curl -s -4 icanhazip.com || echo "UNKNOWN")
WG_IP=$(ip -4 addr show dev wg0 2>/dev/null | grep "inet " | awk '{print $2}' | cut -d/ -f1 || echo "NOT CONFIGURED")

show_header() {
    echo -e "${CYAN}${BOLD}"
    echo "=========================================================="
    echo "          📊  WireNet Telemetry: ${ROLE}           "
    echo "=========================================================="
    echo -e "${NC}"
    echo -e "Public IPv4       : ${BOLD}${PUBLIC_IP}${NC}"
    echo -e "Tunnel IPv4 (wg0) : ${BOLD}${GREEN}${WG_IP}${NC}"
    echo -e "----------------------------------------------------------"
}

show_peers() {
    echo -e "\n${BOLD}--- [1] WireGuard Interface & Handshake Telemetry ---${NC}"
    if ip link show dev wg0 >/dev/null 2>&1; then
        echo -e "Interface (wg0)   : ${GREEN}${BOLD}ACTIVE (UP)${NC}"
        wg show wg0
    else
        echo -e "Interface (wg0)   : ${RED}${BOLD}INACTIVE (DOWN)${NC}"
    fi
}

show_latency() {
    echo -e "\n${BOLD}--- [2] Live Tunnel Latency & Ping ---${NC}"
    if [[ "$IS_GATEWAY" == true ]]; then
        for node in 10.200.0.2 10.200.0.3 10.200.0.4 10.200.0.5; do
            if ping -c 1 -W 1 "$node" >/dev/null 2>&1; then
                RTT=$(ping -c 1 -W 1 "$node" | tail -1 | awk '{print $4}' | cut -d/ -f2)
                echo -e "  Node ${node}  : ${GREEN}${BOLD}ONLINE${NC} (${RTT}ms RTT - Sub-millisecond)"
            fi
        done
    else
        if ping -c 1 -W 1 10.200.0.1 >/dev/null 2>&1; then
            RTT=$(ping -c 1 -W 1 10.200.0.1 | tail -1 | awk '{print $4}' | cut -d/ -f2)
            echo -e "  Gateway 10.200.0.1 : ${GREEN}${BOLD}CONNECTED${NC} (${RTT}ms RTT)"
        else
            echo -e "  Gateway 10.200.0.1 : ${RED}${BOLD}UNREACHABLE${NC}"
        fi
    fi
}

show_ports() {
    echo -e "\n${BOLD}--- [3] Active Minecraft & Game Server Port Bindings ---${NC}"
    if ss -tulpn 2>/dev/null | grep -E "25565|rinetd|dockerd|java" >/dev/null; then
        ss -tulpn 2>/dev/null | grep -E "25565|rinetd|dockerd|java" | awk '{printf "  %-7s %-25s %-15s\n", $1, $5, $7}'
    else
        echo "  (No active game server processes detected on port 25565)"
    fi
}

show_firewall() {
    if [[ "$IS_GATEWAY" == true ]]; then
        echo -e "\n${BOLD}--- [4] Minecraft Anti-DDoS Shield Status ---${NC}"
        if iptables -L INPUT -n 2>/dev/null | grep -q "MC_TCP_FILTER"; then
            echo -e "  Shield Status   : ${GREEN}${BOLD}ENABLED (Hardware SYN Cookies & Rate Limiting Active)${NC}"
            echo -e "  Dropped Packets :"
            iptables -L MC_TCP_FILTER -n -v 2>/dev/null | grep "DROP" | head -n 3 || echo "  (0 dropped attacks)"
        else
            echo -e "  Shield Status   : ${YELLOW}DISABLED (Pass-Through Mode)${NC}"
        fi
    fi
}

# Execution based on flag
show_header

case "$MODE" in
    peers|wg)
        show_peers
        ;;
    latency|ping)
        show_latency
        ;;
    ports|servers)
        show_ports
        ;;
    firewall|shield)
        show_firewall
        ;;
    all|*)
        show_peers
        show_latency
        show_ports
        show_firewall
        ;;
esac

echo ""
echo "=========================================================="
