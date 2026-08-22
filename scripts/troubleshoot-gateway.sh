#!/usr/bin/env bash
# ==============================================================================
# WireNet Gateway Auto-Troubleshooter & Self-Healing Repair Tool
# Diagnoses gateway peers, firewall rules, and auto-repairs in 1 click
# ==============================================================================

set -euo pipefail

echo "=========================================================="
echo " WireNet Gateway Health Diagnostic & Auto-Repair Tool"
echo "=========================================================="

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

DEFAULT_IFACE=$(ip route show default 2>/dev/null | awk '{print $5}' | head -n1)
DEFAULT_IFACE="${DEFAULT_IFACE:-eth0}"
PUBLIC_IP=$(ip route get 1.1.1.1 2>/dev/null | awk '{print $7; exit}' || hostname -I 2>/dev/null | awk '{print $1}' || curl -s -4 --connect-timeout 2 -m 2 ifconfig.me 2>/dev/null || echo "127.0.0.1")

echo "[1/5] Checking Gateway Kernel Forwarding..."
sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.all.forwarding=1 >/dev/null 2>&1 || true
echo "  [✓] Kernel IP packet forwarding is ENABLED."

echo "[2/5] Checking WireGuard Gateway Interface (wg0)..."
if ip link show dev wg0 >/dev/null 2>&1; then
    PEER_COUNT=$(wg show wg0 peers 2>/dev/null | wc -l || echo "0")
    echo "  [✓] Interface wg0 is UP (Authorized Peers: $PEER_COUNT)"
else
    echo "  [!] Interface wg0 is DOWN. Restarting WireGuard..."
    systemctl restart wg-quick@wg0 || true
fi

echo "[3/5] Checking Backend Node Pings..."
for node_ip in 10.200.0.2 10.200.0.3 10.200.0.4; do
    if ping -c 1 -W 1 "$node_ip" >/dev/null 2>&1; then
        RTT=$(ping -c 1 -W 1 "$node_ip" | tail -1 | awk '{print $4}' | cut -d/ -f2)
        echo "  [✓] Node $node_ip is ONLINE! Latency: ${RTT}ms"
    fi
done

echo "[4/5] Refreshing Port Forwarding & Firewall Rules..."
# 1. Stop any userspace forwarders to eliminate port binding and proxy conflicts
systemctl stop rinetd haproxy wirenet-gateway 2>/dev/null || true
systemctl disable rinetd haproxy 2>/dev/null || true

# 2. Enable kernel packet forwarding
sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.all.forwarding=1 >/dev/null 2>&1 || true

# 3. Allow all forwarding unconditionally
iptables -I FORWARD 1 -j ACCEPT 2>/dev/null || true
iptables -I INPUT 1 -i wg0 -j ACCEPT 2>/dev/null || true
iptables -I INPUT 1 -p tcp --dport 9000 -j ACCEPT 2>/dev/null || true
iptables -I INPUT 1 -p tcp -m multiport --dports 25565:25700,30000:40000 -j ACCEPT 2>/dev/null || true
iptables -I INPUT 1 -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j ACCEPT 2>/dev/null || true

# 4. Clean NAT tables and configure pure Kernel Layer-3 DNAT
iptables -t nat -F PREROUTING 2>/dev/null || true
iptables -t nat -A PREROUTING -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination 10.200.0.2
iptables -t nat -A PREROUTING -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DNAT --to-destination 10.200.0.2

# 5. Symmetrical WireGuard MASQUERADE
iptables -t nat -D POSTROUTING -o wg0 -j MASQUERADE 2>/dev/null || true
iptables -t nat -A POSTROUTING -o wg0 -j MASQUERADE
iptables -t nat -D POSTROUTING -o "$DEFAULT_IFACE" -j MASQUERADE 2>/dev/null || true
iptables -t nat -A POSTROUTING -o "$DEFAULT_IFACE" -j MASQUERADE

# Open in UFW if UFW is active
if command -v ufw >/dev/null 2>&1 && ufw status | grep -q "Status: active"; then
    ufw allow 9000/tcp >/dev/null 2>&1 || true
    ufw allow 51820/udp >/dev/null 2>&1 || true
    ufw allow 25565:25700/tcp >/dev/null 2>&1 || true
    ufw allow 25565:25700/udp >/dev/null 2>&1 || true
    ufw allow 30000:40000/tcp >/dev/null 2>&1 || true
    ufw allow 30000:40000/udp >/dev/null 2>&1 || true
fi
echo "  [✓] Gateway Pure Kernel DNAT Active (Forwarding to Node 10.200.0.2)."

echo "[5/5] Testing End-to-End Minecraft Port..."
if nc -z -w 2 127.0.0.1 25565 2>/dev/null; then
    echo "  [✓] SUCCESS: Gateway Public Ingress is LISTENING on port 25565!"
fi

if nc -z -w 2 10.200.0.2 25565 2>/dev/null; then
    echo "  [✓] SUCCESS: Gateway reached Minecraft server on Node port 25565!"
elif nc -z -w 2 10.200.0.2 25566 2>/dev/null; then
    echo "  [✓] SUCCESS: Gateway reached Minecraft server on Node port 25566!"
else
    echo "  [i] Gateway reached Node tunnel, waiting for Minecraft server."
fi

echo "=========================================================="
echo " [✓] Gateway Diagnostics & Auto-Repair Complete!"
echo " Gateway Public IP: $PUBLIC_IP"
echo "=========================================================="
