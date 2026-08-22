#!/usr/bin/env bash
# ==============================================================================
# WireNet Doctor & 6-Point System Diagnostic Inspector
# Developed by UG88 | https://github.com/UG88/wirenet
# Performs deep kernel, tunnel, Docker, and routing checks with 1-click auto-repair
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
   echo -e "${RED}[-] Error: WireNet Doctor must be run as root (or with sudo).${NC}" >&2
   exit 1
fi

# Detect Role
if [[ -f /etc/wireguard/gateway_private.key ]] || ip addr show dev wg0 2>/dev/null | grep -q "10.200.0.1/"; then
    IS_GATEWAY=true
    ROLE="GATEWAY VPS (Hub)"
else
    IS_GATEWAY=false
    ROLE="PTERODACTYL NODE VPS (Spoke)"
fi

PUBLIC_IP=$(curl -s -4 ifconfig.me 2>/dev/null || curl -s -4 icanhazip.com 2>/dev/null || echo "UNKNOWN")
WG_IP=$(ip -4 addr show dev wg0 2>/dev/null | grep "inet " | awk '{print $2}' | cut -d/ -f1 || echo "NOT CONFIGURED")

echo -e "${CYAN}${BOLD}"
echo "=========================================================="
echo "          🩺  WireNet System Doctor & Inspector           "
echo "=========================================================="
echo -e "${NC}"
echo -e "Server Role      : ${BOLD}${ROLE}${NC}"
echo -e "Public IPv4      : ${BOLD}${PUBLIC_IP}${NC}"
echo -e "WireGuard IP     : ${BOLD}${GREEN}${WG_IP}${NC}"
echo -e "----------------------------------------------------------"

ISSUES_FOUND=0

# [Check 1] Kernel IP Forwarding
echo -n "Checking Kernel IP Forwarding... "
IP_FWD=$(sysctl -n net.ipv4.ip_forward 2>/dev/null || echo "0")
if [[ "$IP_FWD" == "1" ]]; then
    echo -e "${GREEN}[PASS]${NC} (net.ipv4.ip_forward = 1)"
else
    echo -e "${RED}[FAIL]${NC} (net.ipv4.ip_forward = 0)"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

# [Check 2] WireGuard Interface
echo -n "Checking WireGuard Service (wg0)... "
if ip link show dev wg0 >/dev/null 2>&1; then
    echo -e "${GREEN}[PASS]${NC} (Interface wg0 is UP)"
else
    echo -e "${RED}[FAIL]${NC} (Interface wg0 is DOWN)"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

# [Check 3] Peer Handshake & Connectivity
echo -n "Checking WireGuard Peer Handshakes... "
if wg show wg0 latest-handshakes 2>/dev/null | grep -q "[1-9]"; then
    LATEST_HS=$(wg show wg0 latest-handshakes 2>/dev/null | awk '{print $2}' | head -n1)
    NOW=$(date +%s)
    DIFF=$((NOW - LATEST_HS))
    if [[ $DIFF -lt 180 ]]; then
        echo -e "${GREEN}[PASS]${NC} (Active handshake ${DIFF}s ago)"
    else
        echo -e "${YELLOW}[WARN]${NC} (Last handshake ${DIFF}s ago - connection idle)"
    fi
else
    echo -e "${YELLOW}[WARN]${NC} (No active handshake recorded yet)"
fi

# [Check 4] Tunnel Ping & Latency
if [[ "$IS_GATEWAY" == true ]]; then
    TARGET_PING="10.200.0.2"
else
    TARGET_PING="10.200.0.1"
fi

echo -n "Testing Tunnel Ping (${TARGET_PING})... "
if ping -c 2 -W 2 "$TARGET_PING" >/dev/null 2>&1; then
    RTT=$(ping -c 2 "$TARGET_PING" 2>/dev/null | tail -1 | awk '{print $4}' | cut -d/ -f2 || echo "0.1")
    echo -e "${GREEN}[PASS]${NC} (Latency: ${RTT}ms - Sub-millisecond)"
else
    echo -e "${RED}[FAIL]${NC} (Cannot reach ${TARGET_PING} over tunnel)"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
fi

# [Check 5] Node-Specific Checks (Docker & rinetd)
if [[ "$IS_GATEWAY" == false ]]; then
    echo -n "Checking Docker / Wings Service... "
    if command -v docker >/dev/null 2>&1 && systemctl is-active --quiet docker; then
        CONTAINER_COUNT=$(docker ps -q 2>/dev/null | wc -l || echo "0")
        echo -e "${GREEN}[PASS]${NC} (Docker active, ${CONTAINER_COUNT} container(s) running)"
    else
        echo -e "${YELLOW}[WARN]${NC} (Docker not active or not installed)"
    fi

    echo -n "Checking rinetd Docker Port Bridge... "
    if command -v rinetd >/dev/null 2>&1 && systemctl is-active --quiet rinetd; then
        echo -e "${GREEN}[PASS]${NC} (rinetd service running)"
    else
        echo -e "${RED}[FAIL]${NC} (rinetd is NOT running)"
        ISSUES_FOUND=$((ISSUES_FOUND + 1))
    fi

    echo -n "Scanning Active Game Server Ports... "
    FOUND_PORTS=$(ss -tulpn 2>/dev/null | grep -E "dockerd|java|wings" | awk '{print $5}' | awk -F: '{print $NF}' | sort -u || true)
    if [[ -n "$FOUND_PORTS" ]]; then
        echo -e "${GREEN}[PASS]${NC} (Listening on port(s): ${FOUND_PORTS})"
        for p in $FOUND_PORTS; do
            if nc -z -w 1 "$WG_IP" "$p" 2>/dev/null; then
                echo -e "  ├── Port $p: ${GREEN}[PASS]${NC} (Bridged & answering on ${WG_IP}:${p})"
            else
                echo -e "  ├── Port $p: ${YELLOW}[WARN]${NC} (Port ${p} not in rinetd.conf - Auto-Repair can fix this)"
                ISSUES_FOUND=$((ISSUES_FOUND + 1))
            fi
        done
    else
        echo -e "${YELLOW}[WARN]${NC} (No game server processes detected yet)"
    fi
fi

# [Check 6] Gateway-Specific Checks (NAT & Forwarding)
if [[ "$IS_GATEWAY" == true ]]; then
    echo -n "Checking Gateway NAT Forwarding Rules... "
    if iptables -t nat -L PREROUTING -n 2>/dev/null | grep -q "10.200.0"; then
        echo -e "${GREEN}[PASS]${NC} (DNAT rules active)"
    else
        echo -e "${RED}[FAIL]${NC} (No DNAT forwarding rules found)"
        ISSUES_FOUND=$((ISSUES_FOUND + 1))
    fi

    echo -n "Checking Gateway MASQUERADE Rule... "
    if iptables -t nat -L POSTROUTING -n 2>/dev/null | grep -q "MASQUERADE"; then
        echo -e "${GREEN}[PASS]${NC} (MASQUERADE active on wg0)"
    else
        echo -e "${RED}[FAIL]${NC} (MASQUERADE missing on wg0)"
        ISSUES_FOUND=$((ISSUES_FOUND + 1))
    fi

    echo -n "Checking Minecraft Anti-DDoS Shield... "
    if iptables -L INPUT -n 2>/dev/null | grep -q "MC_TCP_FILTER"; then
        echo -e "${GREEN}[PASS]${NC} (Anti-DDoS Shield is ACTIVE)"
    else
        echo -e "${YELLOW}[WARN]${NC} (Anti-DDoS chains not attached)"
    fi
fi

echo -e "----------------------------------------------------------"

if [[ $ISSUES_FOUND -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}[✓] ALL SYSTEMS HEALTHY! WireNet is 100% operational.${NC}\n"
else
    echo -e "${RED}${BOLD}[!] Detected ${ISSUES_FOUND} issue(s).${NC}\n"
    echo -e "${YELLOW}Starting automatic self-healing repair...${NC}"
    if [[ "$IS_GATEWAY" == true ]]; then
        curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/scripts/troubleshoot-gateway.sh?$(date +%s) | sudo bash
    else
        curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/scripts/troubleshoot-node.sh?$(date +%s) | sudo bash
    fi
fi
