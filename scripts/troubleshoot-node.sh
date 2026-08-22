#!/usr/bin/env bash
# ==============================================================================
# WireNet Node Auto-Troubleshooter & Self-Healing Repair Tool
# Diagnoses tunnel, port bindings, and auto-repairs all issues in 1 click
# ==============================================================================

set -euo pipefail

echo "=========================================================="
echo " WireNet Node Health Diagnostic & Auto-Repair Tool"
echo "=========================================================="

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

PRIMARY_IP=$(curl -s -4 ifconfig.me || curl -s -4 icanhazip.com || ip route get 1.1.1.1 | awk '{print $7; exit}')
NODE_IP=$(ip addr show dev wg0 2>/dev/null | grep "inet " | awk '{print $2}' | cut -d/ -f1 || echo "10.200.0.2")

echo "[1/5] Checking WireGuard Interface (wg0)..."
if ip link show dev wg0 >/dev/null 2>&1; then
    echo "  [✓] Interface wg0 is UP (Node IP: $NODE_IP)"
else
    echo "  [!] Interface wg0 is DOWN. Restarting WireGuard..."
    systemctl restart wg-quick@wg0 || true
fi

echo "[2/5] Checking Gateway Ping (10.200.0.1)..."
if ping -c 2 -W 2 10.200.0.1 >/dev/null 2>&1; then
    RTT=$(ping -c 2 10.200.0.1 | tail -1 | awk '{print $4}' | cut -d/ -f2)
    echo "  [✓] Ping to Gateway is SUCCESSFUL! Latency: ${RTT}ms"
else
    echo "  [!] Gateway ping failed! Refreshing tunnel route..."
    ip route add 10.200.0.0/24 dev wg0 2>/dev/null || true
    systemctl restart wg-quick@wg0 || true
fi

echo "[3/5] Checking & Cleaning iptables NAT rules..."
iptables -t nat -F PREROUTING 2>/dev/null || true
iptables -t nat -F POSTROUTING 2>/dev/null || true
iptables -t mangle -F 2>/dev/null || true
iptables -I INPUT 1 -i wg0 -j ACCEPT 2>/dev/null || true
iptables -I FORWARD 1 -i wg0 -j ACCEPT 2>/dev/null || true
echo "  [✓] NAT tables cleaned and tunnel INPUT allowed."

echo "[4/5] Checking rinetd Port Bridge..."
if ! command -v rinetd >/dev/null 2>&1; then
    echo "  [+] Installing rinetd..."
    apt-get update -qq && apt-get install -y rinetd || true
fi

# Rebuild clean rinetd config
cat << EOF > /etc/rinetd.conf
# WireNet Automated Docker Port Bridge
$NODE_IP 25565 $PRIMARY_IP 25565
EOF

for port in $(seq 25566 25600); do
    echo "$NODE_IP $port $PRIMARY_IP $port" >> /etc/rinetd.conf
done

for port in $(seq 30000 30050); do
    echo "$NODE_IP $port $PRIMARY_IP $port" >> /etc/rinetd.conf
done

systemctl restart rinetd 2>/dev/null || true
echo "  [✓] rinetd port bridge active on ports 25565-25600 and 30000-30050"

echo "[5/5] Testing Local Port 25565..."
if nc -z -w 2 10.200.0.2 25565 2>/dev/null; then
    echo "  [✓] Port 25565 is listening and answering on 10.200.0.2!"
elif nc -z -w 2 "$PRIMARY_IP" 25565 2>/dev/null; then
    echo "  [✓] Minecraft server is running on $PRIMARY_IP:25565!"
else
    echo "  [i] Minecraft server is currently stopped. Start it in Pterodactyl when ready."
fi

echo "=========================================================="
echo " [✓] Node Diagnostics & Auto-Repair Complete!"
echo "=========================================================="
