#!/usr/bin/env bash
# ==============================================================================
# Complete WireNet Uninstaller & Cleaner
# Completely stops and removes WireNet interfaces, configs, keys, and firewall rules
# ==============================================================================

set -euo pipefail

echo "=========================================================="
echo " Stopping and Removing WireNet Configuration"
echo "=========================================================="

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

# 1. Stop and disable WireGuard interface
echo "[+] Stopping WireGuard service (wg-quick@wg0)..."
systemctl stop wg-quick@wg0 2>/dev/null || true
systemctl disable wg-quick@wg0 2>/dev/null || true
wg-quick down wg0 2>/dev/null || true

# 2. Flush custom policy routes or rules
echo "[+] Cleaning up policy routing rules..."
ip rule del from 10.200.0.2 table 200 2>/dev/null || true
ip route flush table 200 2>/dev/null || true

# 3. Clean up firewall chains
DEFAULT_IFACE=$(ip route show default 2>/dev/null | awk '{print $5}' | head -n1)
DEFAULT_IFACE="${DEFAULT_IFACE:-eth0}"
iptables -D INPUT -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25600,30000:40000 -j MC_TCP_FILTER 2>/dev/null || true
iptables -D INPUT -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25600,30000:40000 -j MC_UDP_FILTER 2>/dev/null || true
iptables -F MC_TCP_FILTER 2>/dev/null || true
iptables -X MC_TCP_FILTER 2>/dev/null || true
iptables -F MC_UDP_FILTER 2>/dev/null || true
iptables -X MC_UDP_FILTER 2>/dev/null || true

# 4. Remove WireGuard configuration and keys
echo "[+] Removing /etc/wireguard directory and keys..."
rm -rf /etc/wireguard
rm -f /etc/sysctl.d/99-wirenet.conf /etc/sysctl.d/98-minecraft-security.conf

# 5. Reload systemd
systemctl daemon-reload
systemctl reset-failed 2>/dev/null || true

echo "=========================================================="
echo " [✓] WireNet has been completely and cleanly removed!"
echo "=========================================================="
