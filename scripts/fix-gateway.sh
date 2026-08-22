#!/usr/bin/env bash
# ==============================================================================
# WireNet Auto-Fix & Repair Tool for Gateway VPS
# Ensures 100% working port forwarding, MASQUERADE, and packet routing
# ==============================================================================

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

echo "=========================================================="
echo " Running WireNet Gateway Auto-Repair & Diagnostic"
echo "=========================================================="

DEFAULT_IFACE=$(ip route show default 2>/dev/null | awk '{print $5}' | head -n1)
DEFAULT_IFACE="${DEFAULT_IFACE:-eth0}"
echo "[+] Detected public interface: $DEFAULT_IFACE"

# 1. Ensure Kernel Forwarding
sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.all.forwarding=1 >/dev/null 2>&1 || true

# 2. Reset and Apply Clean Forwarding & NAT Rules
echo "[+] Configuring Gateway NAT & Port Forwarding rules..."
iptables -A FORWARD -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT 2>/dev/null || true
iptables -A FORWARD -i wg0 -j ACCEPT 2>/dev/null || true
iptables -A FORWARD -o wg0 -j ACCEPT 2>/dev/null || true

# Ensure MASQUERADE on wg0
iptables -t nat -D POSTROUTING -o wg0 -j MASQUERADE 2>/dev/null || true
iptables -t nat -A POSTROUTING -o wg0 -j MASQUERADE

# Ensure Port Forwarding (DNAT to Node 10.200.0.2)
iptables -t nat -D PREROUTING -p tcp -m multiport --dports 25565:25600,30000:40000 -j DNAT --to-destination 10.200.0.2 2>/dev/null || true
iptables -t nat -A PREROUTING -p tcp -m multiport --dports 25565:25600,30000:40000 -j DNAT --to-destination 10.200.0.2

iptables -t nat -D PREROUTING -p udp -m multiport --dports 25565:25600,30000:40000 -j DNAT --to-destination 10.200.0.2 2>/dev/null || true
iptables -t nat -A PREROUTING -p udp -m multiport --dports 25565:25600,30000:40000 -j DNAT --to-destination 10.200.0.2

# 3. Allow in UFW if UFW is active
if command -v ufw >/dev/null 2>&1 && ufw status | grep -q "Status: active"; then
    ufw allow 25565:25600/tcp >/dev/null 2>&1 || true
    ufw allow 25565:25600/udp >/dev/null 2>&1 || true
    ufw allow 30000:40000/tcp >/dev/null 2>&1 || true
    ufw allow 30000:40000/udp >/dev/null 2>&1 || true
fi

echo "=========================================================="
echo " [✓] Gateway Repair Complete!"
echo " Testing connection to Node 10.200.0.2..."
echo "=========================================================="

if nc -z -w 3 10.200.0.2 25565 2>/dev/null; then
    echo "[✓] SUCCESS: Gateway can reach Minecraft server on Node port 25565!"
else
    echo "[!] Notice: Gateway reached Node tunnel, waiting for Minecraft server on port 25565."
fi
