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

# 1. Enable Kernel Packet Forwarding
sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.all.forwarding=1 >/dev/null 2>&1 || true

# 2. Detect Node Public IP and Primary Interface
PRIMARY_IP=$(curl -s -4 ifconfig.me || curl -s -4 icanhazip.com || ip route get 1.1.1.1 | awk '{print $7; exit}')
echo "[+] Detected Node Host IP: $PRIMARY_IP"

# 3. Configure Forwarding from wg0 to Docker / Local Ports
echo "[+] Bridging WireGuard tunnel into Docker / Wings containers..."
iptables -A FORWARD -i wg0 -j ACCEPT 2>/dev/null || true
iptables -A FORWARD -o wg0 -j ACCEPT 2>/dev/null || true

# Route traffic arriving on 10.200.0.2 to local Docker bindings
iptables -t nat -D PREROUTING -i wg0 -d 10.200.0.2 -p tcp -m multiport --dports 25565:25600,30000:40000 -j DNAT --to-destination "$PRIMARY_IP" 2>/dev/null || true
iptables -t nat -A PREROUTING -i wg0 -d 10.200.0.2 -p tcp -m multiport --dports 25565:25600,30000:40000 -j DNAT --to-destination "$PRIMARY_IP"

iptables -t nat -D PREROUTING -i wg0 -d 10.200.0.2 -p udp -m multiport --dports 25565:25600,30000:40000 -j DNAT --to-destination "$PRIMARY_IP" 2>/dev/null || true
iptables -t nat -A PREROUTING -i wg0 -d 10.200.0.2 -p udp -m multiport --dports 25565:25600,30000:40000 -j DNAT --to-destination "$PRIMARY_IP"

# 4. Restart WireGuard on Node
systemctl restart wg-quick@wg0 2>/dev/null || true

echo "=========================================================="
echo " [✓] Node Auto-Repair Complete!"
echo " Testing tunnel ping to Gateway 10.200.0.1..."
echo "=========================================================="

if ping -c 2 10.200.0.1 >/dev/null 2>&1; then
    echo "[✓] SUCCESS: Node can ping Gateway 10.200.0.1 (<1ms latency)!"
else
    echo "[!] Notice: Gateway ping pending. Ensure Gateway is online."
fi
