#!/usr/bin/env bash
# ==============================================================================
# WireNet Node Routing Fix (Enables Docker Forwarding + CONNMARK Policy Routing)
# ==============================================================================

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

echo "[+] Enabling packet forwarding and loose reverse path filtering in kernel..."
sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.all.forwarding=1 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.all.rp_filter=2 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.default.rp_filter=2 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.wg0.rp_filter=2 >/dev/null 2>&1 || true

echo "[+] Configuring iptables connection-tracking marks for Docker/Wings containers..."
iptables -A FORWARD -i wg0 -j ACCEPT 2>/dev/null || true
iptables -A FORWARD -o wg0 -j ACCEPT 2>/dev/null || true

iptables -t mangle -D PREROUTING -i wg0 -j CONNMARK --set-mark 1 2>/dev/null || true
iptables -t mangle -A PREROUTING -i wg0 -j CONNMARK --set-mark 1

iptables -t mangle -D PREROUTING -m connmark --mark 1 -j CONNMARK --restore-mark 2>/dev/null || true
iptables -t mangle -A PREROUTING -m connmark --mark 1 -j CONNMARK --restore-mark

iptables -t mangle -D OUTPUT -m connmark --mark 1 -j CONNMARK --restore-mark 2>/dev/null || true
iptables -t mangle -A OUTPUT -m connmark --mark 1 -j CONNMARK --restore-mark

echo "[+] Setting up routing table 200..."
ip rule del fwmark 1 table 200 2>/dev/null || true
ip rule add fwmark 1 table 200 2>/dev/null || true

ip route flush table 200 2>/dev/null || true
ip route add default via 10.200.0.1 dev wg0 table 200 2>/dev/null || true

echo "=========================================================="
echo " [✓] Node routing fix applied successfully!"
echo " Docker containers can now receive and reply to game traffic!"
echo "=========================================================="
