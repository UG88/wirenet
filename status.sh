#!/usr/bin/env bash
# ==============================================================================
# WireNet Status & Telemetry Dashboard
# Displays live tunnel health, active peers, handshakes, traffic, and game ports
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

echo -e "${CYAN}${BOLD}"
echo "=========================================================="
echo "          📊  WireNet Live Telemetry Dashboard           "
echo "=========================================================="
echo -e "${NC}"

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

echo -e "Server Role       : ${BOLD}${ROLE}${NC}"
echo -e "Public IPv4       : ${BOLD}${PUBLIC_IP}${NC}"
echo -e "Tunnel IPv4 (wg0) : ${BOLD}${GREEN}${WG_IP}${NC}"
echo -e "----------------------------------------------------------"

# 1. WireGuard Interface Status
if ip link show dev wg0 >/dev/null 2>&1; then
    echo -e "Interface (wg0)   : ${GREEN}${BOLD}ACTIVE (UP)${NC}"
else
    echo -e "Interface (wg0)   : ${RED}${BOLD}INACTIVE (DOWN)${NC}"
    echo "  To start: sudo systemctl restart wg-quick@wg0"
    exit 1
fi

# 2. Peer & Handshake Telemetry
echo ""
echo -e "${BOLD}--- WireGuard Peer & Handshake Telemetry ---${NC}"
wg show wg0

# 3. Live Latency
echo ""
echo -e "${BOLD}--- Tunnel Latency Telemetry ---${NC}"
if [[ "$IS_GATEWAY" == true ]]; then
    for node in 10.200.0.2 10.200.0.3 10.200.0.4; do
        if ping -c 1 -W 1 "$node" >/dev/null 2>&1; then
            RTT=$(ping -c 1 -W 1 "$node" | tail -1 | awk '{print $4}' | cut -d/ -f2)
            echo -e "  Node ${node}  : ${GREEN}${BOLD}ONLINE${NC} (${RTT}ms RTT)"
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

# 4. Listening Game Ports
echo ""
echo -e "${BOLD}--- Active Minecraft & Game Server Ports ---${NC}"
if ss -tulpn 2>/dev/null | grep -E "25565|rinetd|dockerd|java" >/dev/null; then
    ss -tulpn 2>/dev/null | grep -E "25565|rinetd|dockerd|java" | awk '{printf "  %-7s %-25s %-15s\n", $1, $5, $7}'
else
    echo "  (No active game server processes detected)"
fi

# 5. Anti-DDoS Shield (If Gateway)
if [[ "$IS_GATEWAY" == true ]]; then
    echo ""
    echo -e "${BOLD}--- Minecraft Anti-DDoS Shield ---${NC}"
    if iptables -L INPUT -n 2>/dev/null | grep -q "MC_TCP_FILTER"; then
        echo -e "  Shield Status   : ${GREEN}${BOLD}ENABLED (Hardware SYN Cookies & Rate-Limiting Active)${NC}"
    else
        echo -e "  Shield Status   : ${YELLOW}DISABLED${NC}"
    fi
fi

echo ""
echo "=========================================================="
echo -e " Run ${CYAN}curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/doctor.sh | sudo bash${NC} for full doctor scan."
echo "=========================================================="
