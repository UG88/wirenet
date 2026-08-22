#!/usr/bin/env bash
# ==============================================================================
# WireNet Custom Port Forwarder & Translator (Gateway -> Backend Node)
# Maps custom Public Gateway ports to custom Backend Node ports
# Example: Gateway:25567 -> Node 2 (10.200.0.3):25565
# ==============================================================================

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

PUBLIC_PORT="${1:-}"
DEST_NODE="${2:-}"
DEST_PORT="${3:-}"

if [[ -z "$PUBLIC_PORT" || -z "$DEST_NODE" || -z "$DEST_PORT" ]]; then
    echo "=========================================================="
    echo " WireNet Custom Port Forwarder"
    echo "=========================================================="
    echo "Usage: sudo bash forward-port.sh <GATEWAY_PORT> <NODE_IP> <LOCAL_PORT>"
    echo ""
    echo "Example (Map Gateway Port 25567 to Node 2 Port 25565):"
    echo "  sudo bash forward-port.sh 25567 10.200.0.3 25565"
    echo "=========================================================="
    exit 1
fi

DEFAULT_IFACE=$(ip route show default 2>/dev/null | awk '{print $5}' | head -n1)
DEFAULT_IFACE="${DEFAULT_IFACE:-eth0}"

echo "[+] Forwarding Public Port ${PUBLIC_PORT} -> ${DEST_NODE}:${DEST_PORT} on interface ${DEFAULT_IFACE}..."

# Enable kernel packet forwarding
sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true

# Apply DNAT translation rules
iptables -t nat -D PREROUTING -p tcp --dport "$PUBLIC_PORT" -j DNAT --to-destination "${DEST_NODE}:${DEST_PORT}" 2>/dev/null || true
iptables -t nat -A PREROUTING -p tcp --dport "$PUBLIC_PORT" -j DNAT --to-destination "${DEST_NODE}:${DEST_PORT}"

iptables -t nat -D PREROUTING -p udp --dport "$PUBLIC_PORT" -j DNAT --to-destination "${DEST_NODE}:${DEST_PORT}" 2>/dev/null || true
iptables -t nat -A PREROUTING -p udp --dport "$PUBLIC_PORT" -j DNAT --to-destination "${DEST_NODE}:${DEST_PORT}"

# Ensure MASQUERADE on wg0 for return path
iptables -t nat -D POSTROUTING -o wg0 -j MASQUERADE 2>/dev/null || true
iptables -t nat -A POSTROUTING -o wg0 -j MASQUERADE

# Allow in UFW if active
if command -v ufw >/dev/null 2>&1 && ufw status | grep -q "Status: active"; then
    ufw allow "${PUBLIC_PORT}/tcp" >/dev/null 2>&1 || true
    ufw allow "${PUBLIC_PORT}/udp" >/dev/null 2>&1 || true
fi

echo "=========================================================="
echo " [✓] Port translation active!"
echo " Gateway Port ${PUBLIC_PORT} (TCP/UDP) ──► ${DEST_NODE}:${DEST_PORT}"
echo "=========================================================="
