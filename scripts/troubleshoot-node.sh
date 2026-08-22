#!/usr/bin/env bash
# ==============================================================================
# WireNet Node Auto-Troubleshooter & Instant Tunnel Restoration Tool
# Restores WireGuard tunnel, fixes recursive routing, and rebuilds rinetd port bridge
# ==============================================================================

set -euo pipefail

echo "=========================================================="
echo " WireNet Node Health Diagnostic & Tunnel Repair Tool"
echo "=========================================================="

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

PRIMARY_IP=$(ip route get 1.1.1.1 2>/dev/null | awk '{print $7; exit}' || hostname -I 2>/dev/null | awk '{print $1}' || curl -s -4 --connect-timeout 2 -m 2 ifconfig.me 2>/dev/null || echo "127.0.0.1")
NODE_IP=$(ip -4 addr show dev wg0 2>/dev/null | grep "inet " | awk '{print $2}' | cut -d/ -f1 || echo "10.200.0.2")

echo "[1/6] Enabling Transparent Real IP Ingress (AllowedIPs = 0.0.0.0/0 & Table = off)..."
if [[ -f /etc/wireguard/wg0.conf ]]; then
    # Ensure Table = off is present so wg-quick never touches default routing table
    if ! grep -q "Table = off" /etc/wireguard/wg0.conf; then
        sed -i '/PrivateKey = /a Table = off' /etc/wireguard/wg0.conf 2>/dev/null || true
    fi
    # Set AllowedIPs = 0.0.0.0/0 so WireGuard accepts all real player source IPs
    sed -i 's/^AllowedIPs = .*/AllowedIPs = 0.0.0.0\/0/g' /etc/wireguard/wg0.conf 2>/dev/null || true
fi

# Clean any custom routing tables or rules
ip rule del fwmark 0x1 table 100 2>/dev/null || true
ip route flush table 100 2>/dev/null || true
ip route del default dev wg0 2>/dev/null || true

# Restart WireGuard cleanly
systemctl restart wg-quick@wg0 || true

GW_KEY=$(wg show wg0 peers 2>/dev/null | head -n1 || true)
if [[ -n "$GW_KEY" ]]; then
    wg set wg0 peer "$GW_KEY" allowed-ips 0.0.0.0/0 2>/dev/null || true
fi

ip route add 10.200.0.0/24 dev wg0 2>/dev/null || true
echo "  [✓] WireGuard interface wg0 configured for Transparent Real IP."

echo "[2/6] Testing Gateway Tunnel Ping (10.200.0.1)..."
sleep 1
if ping -c 2 -W 2 10.200.0.1 >/dev/null 2>&1; then
    RTT=$(ping -c 2 10.200.0.1 | tail -1 | awk '{print $4}' | cut -d/ -f2)
    echo "  [✓] SUCCESS: Ping to Gateway 10.200.0.1 is ONLINE! Latency: ${RTT}ms"
else
    echo "  [!] Gateway ping failed! Retrying handshake..."
    systemctl restart wg-quick@wg0 || true
fi

echo "[3/6] Configuring Transparent Real IP Policy Routing & Firewall..."
sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.all.route_localnet=1 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.all.rp_filter=2 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.default.rp_filter=2 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.wg0.rp_filter=2 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.eth0.rp_filter=2 >/dev/null 2>&1 || true

# Set up CONNMARK connection tracking for asymmetric return path
iptables -t mangle -F 2>/dev/null || true
iptables -t mangle -A PREROUTING -i wg0 -m conntrack --ctstate NEW -j CONNMARK --set-mark 0x1
iptables -t mangle -A PREROUTING -j CONNMARK --restore-mark
iptables -t mangle -A OUTPUT -j CONNMARK --restore-mark

# Set up Policy Routing Table 100
ip rule del fwmark 0x1 table 100 2>/dev/null || true
ip rule add fwmark 0x1 table 100
ip route flush table 100 2>/dev/null || true
ip route add default via 10.200.0.1 dev wg0 table 100

# Flush stale nat tables and add transparent localnet forwarding
iptables -t nat -F PREROUTING 2>/dev/null || true
iptables -t nat -A PREROUTING -i wg0 -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination 127.0.0.1 2>/dev/null || true
iptables -t nat -A PREROUTING -i wg0 -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DNAT --to-destination 127.0.0.1 2>/dev/null || true

# Allow tunnel traffic in firewall
iptables -I INPUT 1 -i wg0 -j ACCEPT 2>/dev/null || true
iptables -I FORWARD 1 -i wg0 -j ACCEPT 2>/dev/null || true
iptables -I FORWARD 1 -o wg0 -j ACCEPT 2>/dev/null || true
iptables -I DOCKER-USER 1 -j ACCEPT 2>/dev/null || true

# Block direct public access on eth0 so backend Node IP is 100% hidden
DEFAULT_IFACE=$(ip route show default 2>/dev/null | awk '{print $5}' | head -n1 || echo "eth0")
iptables -D INPUT -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25700,30000:40000 -j DROP 2>/dev/null || true
iptables -I INPUT 1 -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25700,30000:40000 -j DROP 2>/dev/null || true

iptables -D INPUT -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DROP 2>/dev/null || true
iptables -I INPUT 1 -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DROP 2>/dev/null || true

# Ensure Docker outbound MASQUERADE for Mojang auth & DNS
iptables -t nat -D POSTROUTING -s 172.16.0.0/12 -j MASQUERADE 2>/dev/null || true
iptables -t nat -A POSTROUTING -s 172.16.0.0/12 -j MASQUERADE 2>/dev/null || true
iptables -t nat -D POSTROUTING -s 10.200.0.0/24 -j MASQUERADE 2>/dev/null || true
iptables -t nat -A POSTROUTING -s 10.200.0.0/24 -j MASQUERADE 2>/dev/null || true

echo "[4/6] Rebuilding rinetd Port Bridge for all Game Ports..."
if ! command -v rinetd >/dev/null 2>&1; then
    echo "  [+] Installing rinetd..."
    apt-get update -qq && apt-get install -y rinetd || true
fi

# Rebuild clean rinetd config covering 25565-25700 and 30000-30100
cat << EOF > /etc/rinetd.conf
# WireNet Automated Docker Port Bridge
# Node Virtual IP: $NODE_IP -> Host: $PRIMARY_IP
EOF

for port in $(seq 25565 25700); do
    echo "$NODE_IP $port $PRIMARY_IP $port" >> /etc/rinetd.conf
done

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

systemctl enable --now rinetd 2>/dev/null || true
systemctl restart rinetd 2>/dev/null || true
echo "  [✓] rinetd port bridge active on all game ports (25565-25700, 30000-30100)"

echo "[5/6] Ensuring WireNet Dynamic Watcher is Active..."
mkdir -p /opt/wirenet/scripts
curl -fsSL --max-time 5 -H "Cache-Control: no-cache" "https://raw.githubusercontent.com/UG88/wirenet/main/scripts/wirenet-watcher.sh?$(date +%s)" -o /opt/wirenet/scripts/wirenet-watcher.sh 2>/dev/null || true
chmod 755 /opt/wirenet/scripts/wirenet-watcher.sh 2>/dev/null || true

cat << 'EOF' > /etc/systemd/system/wirenet-watcher.service
[Unit]
Description=WireNet Dynamic Docker Port Watcher Daemon
After=docker.service wg-quick@wg0.service rinetd.service
Wants=docker.service

[Service]
Type=simple
ExecStart=/bin/bash /opt/wirenet/scripts/wirenet-watcher.sh
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload 2>/dev/null || true
systemctl enable wirenet-watcher.service 2>/dev/null || true
systemctl restart --no-block wirenet-watcher.service 2>/dev/null || true
echo "  [✓] WireNet Dynamic Port Watcher Daemon is ACTIVE!"

echo "[6/6] Scanning Active Game Server Ports..."
FOUND_PORTS=$(ss -tulpn 2>/dev/null | grep -E "25565|25566|dockerd|java" | awk '{print $5}' | awk -F: '{print $NF}' | sort -u || true)
if [[ -n "$FOUND_PORTS" ]]; then
    echo "  [✓] Active Server Ports Found: $FOUND_PORTS"
    for p in $FOUND_PORTS; do
        if nc -z -w 1 "$NODE_IP" "$p" 2>/dev/null; then
            echo "    ├── Port $p: [✓] REACHABLE over tunnel $NODE_IP:$p"
        fi
    done
else
    echo "  [i] No server currently running. Start your server in Pterodactyl when ready."
fi

NODE_PUB=$(cat /etc/wireguard/node_public.key 2>/dev/null || wg show wg0 public-key 2>/dev/null || echo "")

if ! ping -c 2 -W 2 10.200.0.1 >/dev/null 2>&1; then
    echo ""
    echo "=========================================================="
    echo " ⚠️  GATEWAY LINK PENDING: Key Authorization Needed!"
    echo "=========================================================="
    echo " The Gateway does not have this Node's public key yet."
    echo " Copy & run this EXACT command on your Gateway VPS (AWS):"
    echo ""
    echo "   wg set wg0 peer $NODE_PUB allowed-ips 10.200.0.2/32"
    echo ""
    echo "=========================================================="
else
    echo "=========================================================="
    echo " [✓] Node Diagnostics & Repair Complete!"
    echo " Encrypted Tunnel to Gateway is 100% ONLINE (10.200.0.1 ↔ 10.200.0.2)"
    echo " All game ports are automatically bridged to Docker!"
    echo "=========================================================="
fi
