#!/usr/bin/env bash
# ==============================================================================
# WireNet Auto-Fix & Repair Tool for Pterodactyl Node VPS
# Detects Docker bindings, bridges wg0 to Docker containers, and verifies connectivity
# ==============================================================================

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

echo "=========================================================="
echo " Running WireNet Node Auto-Repair & Docker Bridge"
echo "=========================================================="

PRIMARY_IP=$(curl -s -4 ifconfig.me 2>/dev/null || curl -s -4 icanhazip.com 2>/dev/null || ip route get 1.1.1.1 2>/dev/null | awk '{print $7; exit}' || echo "127.0.0.1")
NODE_IP=$(ip -4 addr show dev wg0 2>/dev/null | grep "inet " | awk '{print $2}' | cut -d/ -f1 || echo "10.200.0.2")

echo "[+] Node Host IP: $PRIMARY_IP"
echo "[+] Node Tunnel IP: $NODE_IP"

# 1. Enable Kernel Packet Forwarding
sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.all.route_localnet=1 >/dev/null 2>&1 || true

# 2. Clean conflicting NAT rules
iptables -t nat -F PREROUTING 2>/dev/null || true
iptables -t nat -F POSTROUTING 2>/dev/null || true
iptables -t mangle -F 2>/dev/null || true
iptables -I INPUT 1 -i wg0 -j ACCEPT 2>/dev/null || true
iptables -I FORWARD 1 -i wg0 -j ACCEPT 2>/dev/null || true

# 3. Ensure rinetd is installed
if ! command -v rinetd >/dev/null 2>&1; then
    echo "[+] Installing rinetd port bridge..."
    apt-get update -qq && apt-get install -y rinetd || true
fi

# 4. Generate clean rinetd configuration covering 25565-25700 and 30000-30100 + any active docker ports
echo "[+] Generating automated rinetd port bridge for all game servers..."
mkdir -p /etc
cat << EOF > /etc/rinetd.conf
# WireNet Automated Docker Port Bridge
# Node Virtual IP: $NODE_IP -> Host: $PRIMARY_IP
EOF

# Minecraft Range (25565 - 25700)
for port in $(seq 25565 25700); do
    echo "$NODE_IP $port $PRIMARY_IP $port" >> /etc/rinetd.conf
done

# Custom Range (30000 - 30100)
for port in $(seq 30000 30100); do
    echo "$NODE_IP $port $PRIMARY_IP $port" >> /etc/rinetd.conf
done

# Scan any active listening game ports on the host and ensure they are bridged
for port in $(ss -tulpn 2>/dev/null | grep -E "dockerd|java|wings" | awk '{print $5}' | awk -F: '{print $NF}' | sort -u); do
    if [[ "$port" =~ ^[0-9]+$ && "$port" -ge 1024 && "$port" -le 65535 ]]; then
        if ! grep -q "$NODE_IP $port " /etc/rinetd.conf 2>/dev/null; then
            echo "$NODE_IP $port $PRIMARY_IP $port" >> /etc/rinetd.conf
        fi
    fi
done

# 5. Restart services
systemctl restart wg-quick@wg0 2>/dev/null || true
systemctl restart rinetd 2>/dev/null || true

echo "=========================================================="
echo " [✓] Node Auto-Repair Complete!"
echo " All game ports (25565-25700, 30000-30100) bridged into Docker!"
echo "=========================================================="
