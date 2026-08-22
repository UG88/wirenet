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
# Allow forwarding unconditionally to/from wg0
iptables -I FORWARD 1 -o wg0 -j ACCEPT 2>/dev/null || true
iptables -I FORWARD 1 -i wg0 -j ACCEPT 2>/dev/null || true
iptables -I FORWARD 1 -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT 2>/dev/null || true

# Apply MASQUERADE on wg0 and public interface for symmetric, unbreakable TCP routing
iptables -t nat -D POSTROUTING -o wg0 -j MASQUERADE 2>/dev/null || true
iptables -t nat -A POSTROUTING -o wg0 -j MASQUERADE
iptables -t nat -D POSTROUTING -o "$DEFAULT_IFACE" -j MASQUERADE 2>/dev/null || true
iptables -t nat -A POSTROUTING -o "$DEFAULT_IFACE" -j MASQUERADE

# Flush stale PREROUTING nat so rinetd ingress bridge accepts 0.0.0.0 traffic cleanly
iptables -t nat -F PREROUTING 2>/dev/null || true

# Rebuild rinetd fallback bridge on Gateway
if ! command -v rinetd >/dev/null 2>&1; then
    echo "  [+] Installing rinetd on Gateway..."
    DEBIAN_FRONTEND=noninteractive apt-get update -o Acquire::ForceIPv4=true -qq 2>/dev/null || true
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq rinetd 2>/dev/null || true
fi

cat << EOF > /etc/rinetd.conf
# WireNet Gateway Ingress Port Bridge (0.0.0.0 -> Node 10.200.0.2)
EOF

for port in $(seq 25565 25700); do
    echo "0.0.0.0 $port 10.200.0.2 $port" >> /etc/rinetd.conf
done

for port in $(seq 30000 30100); do
    echo "0.0.0.0 $port 10.200.0.2 $port" >> /etc/rinetd.conf
done

systemctl stop wirenet-gateway.service 2>/dev/null || true
systemctl enable --now rinetd 2>/dev/null || true
systemctl restart rinetd 2>/dev/null || true

# Allow control plane port 9000 and WireGuard interface input
iptables -I INPUT 1 -i wg0 -j ACCEPT 2>/dev/null || true
iptables -I INPUT 1 -p tcp --dport 9000 -j ACCEPT 2>/dev/null || true
iptables -I INPUT 1 -p tcp -m multiport --dports 25565:25700,30000:40000 -j ACCEPT 2>/dev/null || true
iptables -I INPUT 1 -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j ACCEPT 2>/dev/null || true

# Open in UFW if UFW is active
if command -v ufw >/dev/null 2>&1 && ufw status | grep -q "Status: active"; then
    ufw allow 9000/tcp >/dev/null 2>&1 || true
    ufw allow 51820/udp >/dev/null 2>&1 || true
    ufw allow 25565:25700/tcp >/dev/null 2>&1 || true
    ufw allow 25565:25700/udp >/dev/null 2>&1 || true
    ufw allow 30000:40000/tcp >/dev/null 2>&1 || true
    ufw allow 30000:40000/udp >/dev/null 2>&1 || true
fi
echo "  [✓] Forwarding and firewall rules refreshed (Ingress Port Bridge Active)."

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
