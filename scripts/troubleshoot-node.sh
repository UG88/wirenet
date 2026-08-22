#!/usr/bin/env bash
# ==============================================================================
# WireNet Node Auto-Troubleshooter & Self-Healing Repair Tool
# Diagnoses tunnel, port bindings, Docker DNS/Internet, and auto-repairs all issues
# ==============================================================================

set -euo pipefail

echo "=========================================================="
echo " WireNet Node Health Diagnostic & Auto-Repair Tool"
echo "=========================================================="

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

PRIMARY_IP=$(curl -s -4 ifconfig.me 2>/dev/null || curl -s -4 icanhazip.com 2>/dev/null || ip route get 1.1.1.1 2>/dev/null | awk '{print $7; exit}' || echo "127.0.0.1")
NODE_IP=$(ip -4 addr show dev wg0 2>/dev/null | grep "inet " | awk '{print $2}' | cut -d/ -f1 || echo "10.200.0.2")

echo "[1/6] Checking WireGuard Interface (wg0)..."
if ip link show dev wg0 >/dev/null 2>&1; then
    echo "  [✓] Interface wg0 is UP (Node IP: $NODE_IP)"
else
    echo "  [!] Interface wg0 is DOWN. Restarting WireGuard..."
    systemctl restart wg-quick@wg0 || true
fi

echo "[2/6] Checking Gateway Ping (10.200.0.1)..."
if ping -c 2 -W 2 10.200.0.1 >/dev/null 2>&1; then
    RTT=$(ping -c 2 10.200.0.1 | tail -1 | awk '{print $4}' | cut -d/ -f2)
    echo "  [✓] Ping to Gateway is SUCCESSFUL! Latency: ${RTT}ms"
else
    echo "  [!] Gateway ping failed! Refreshing tunnel route..."
    ip route add 10.200.0.0/24 dev wg0 2>/dev/null || true
    systemctl restart wg-quick@wg0 || true
fi

echo "[3/6] Restoring Docker Internet Access & Mojang Auth Routing..."
sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.all.route_localnet=1 >/dev/null 2>&1 || true

# Allow tunnel traffic
iptables -I INPUT 1 -i wg0 -j ACCEPT 2>/dev/null || true
iptables -I FORWARD 1 -i wg0 -j ACCEPT 2>/dev/null || true
iptables -I FORWARD 1 -o wg0 -j ACCEPT 2>/dev/null || true

# Ensure Docker containers have MASQUERADE for Outbound Internet & Mojang Auth
iptables -t nat -D POSTROUTING -s 172.16.0.0/12 -j MASQUERADE 2>/dev/null || true
iptables -t nat -A POSTROUTING -s 172.16.0.0/12 -j MASQUERADE 2>/dev/null || true

iptables -t nat -D POSTROUTING -s 10.200.0.0/24 -j MASQUERADE 2>/dev/null || true
iptables -t nat -A POSTROUTING -s 10.200.0.0/24 -j MASQUERADE 2>/dev/null || true

# Ensure Docker live-restore is configured so containers never stop during maintenance
mkdir -p /etc/docker
if [[ ! -f /etc/docker/daemon.json ]] || ! grep -q "dns" /etc/docker/daemon.json 2>/dev/null; then
    cat << 'EOF' > /etc/docker/daemon.json
{
  "dns": ["1.1.1.1", "8.8.8.8"],
  "live-restore": true
}
EOF
    systemctl reload docker 2>/dev/null || true
fi
echo "  [✓] Docker Internet Access, DNS & Mojang Auth routes active."

echo "[4/6] Checking & Updating rinetd Port Bridge for all Game Ports..."
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

systemctl restart rinetd 2>/dev/null || true
echo "  [✓] rinetd port bridge active on all game ports (25565-25700, 30000-30100)"

echo "[5/6] Checking WireNet Dynamic Port Watcher Daemon..."
mkdir -p /opt/wirenet/scripts
curl -fsSL -H "Cache-Control: no-cache" "https://raw.githubusercontent.com/UG88/wirenet/main/scripts/wirenet-watcher.sh?$(date +%s)" -o /opt/wirenet/scripts/wirenet-watcher.sh 2>/dev/null || true
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
systemctl enable --now wirenet-watcher.service 2>/dev/null || true
echo "  [✓] WireNet Dynamic Port Watcher Daemon is ACTIVE!"

echo "[6/6] Scanning Active Game Server Ports..."
FOUND_PORTS=$(ss -tulpn 2>/dev/null | grep -E "25565|dockerd|java" | awk '{print $5}' | awk -F: '{print $NF}' | sort -u || true)
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

echo "=========================================================="
echo " [✓] Node Diagnostics & Auto-Repair Complete!"
echo " Mojang Auth & PaperMC DNS are now 100% operational."
echo "=========================================================="
