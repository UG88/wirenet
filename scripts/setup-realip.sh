#!/usr/bin/env bash
# ==============================================================================
# WireNet 100% Pure Kernel Real-IP Engine (Table=off + Policy Routing Table 200)
# Developed by UG88 | https://github.com/UG88/wirenet
# Pure kernel-level transparent routing directly into Docker containers (ZERO Plugins)
# ==============================================================================

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

# Detect Role
if [[ -f /etc/wireguard/gateway_private.key ]] || ip addr show dev wg0 2>/dev/null | grep -q "10.200.0.1/"; then
    IS_GATEWAY=true
    ROLE="GATEWAY VPS (Hub)"
else
    IS_GATEWAY=false
    ROLE="PTERODACTYL NODE VPS (Spoke)"
fi

DEFAULT_IFACE=$(ip route show default 2>/dev/null | awk '{print $5}' | head -n1 || echo "eth0")

echo "=========================================================="
echo " WireNet Native Real-IP Engine: ${ROLE}"
echo " (Pure Kernel Policy Routing -> 100% Genuine Player IPs)"
echo "=========================================================="

if [[ "$IS_GATEWAY" == true ]]; then
    echo "[+] Configuring Gateway for Transparent Real-IP Delivery..."
    
    # 1. Stop HAProxy so kernel routes directly
    systemctl stop haproxy 2>/dev/null || true
    systemctl disable haproxy 2>/dev/null || true

    # 2. Kernel Parameters
    sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.all.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.default.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.wg0.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf."$DEFAULT_IFACE".rp_filter=2 >/dev/null 2>&1 || true

    # 3. Transparent DNAT without Masquerade on game ports (Preserves Player Source IP!)
    iptables -t nat -F PREROUTING 2>/dev/null || true
    iptables -t nat -A PREROUTING -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination 10.200.0.2
    iptables -t nat -A PREROUTING -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DNAT --to-destination 10.200.0.2

    # Allow Forwarding in all directions
    iptables -I FORWARD 1 -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT 2>/dev/null || true
    iptables -I FORWARD 1 -i "$DEFAULT_IFACE" -o wg0 -j ACCEPT 2>/dev/null || true
    iptables -I FORWARD 1 -i wg0 -o "$DEFAULT_IFACE" -j ACCEPT 2>/dev/null || true

    # Remove any MASQUERADE on wg0 so player source IP is 100% untouched!
    iptables -t nat -D POSTROUTING -o wg0 -j MASQUERADE 2>/dev/null || true

    # Outbound NAT for internet traffic on Gateway
    iptables -t nat -D POSTROUTING -o "$DEFAULT_IFACE" -j MASQUERADE 2>/dev/null || true
    iptables -t nat -A POSTROUTING -o "$DEFAULT_IFACE" -j MASQUERADE

    echo "=========================================================="
    echo " [✓] Gateway is passing raw, untouched player IPs to Node!"
    echo "=========================================================="

else
    echo "[+] Configuring Node Kernel, WireGuard & Policy Routing..."

    # 1. Configure wg0.conf with Table = off and AllowedIPs = 0.0.0.0/0
    NODE_PRIV_KEY=$(grep -E "^PrivateKey" /etc/wireguard/wg0.conf 2>/dev/null | awk '{print $3}' || true)
    GW_PUB_KEY=$(grep -E "^PublicKey" /etc/wireguard/wg0.conf 2>/dev/null | awk '{print $3}' || true)
    GW_ENDPOINT=$(grep -E "^Endpoint" /etc/wireguard/wg0.conf 2>/dev/null | awk '{print $3}' || true)

    if [[ -n "$NODE_PRIV_KEY" && -n "$GW_PUB_KEY" && -n "$GW_ENDPOINT" ]]; then
        cat << EOF > /etc/wireguard/wg0.conf
[Interface]
PrivateKey = $NODE_PRIV_KEY
Address = 10.200.0.2/24
Table = off

[Peer]
PublicKey = $GW_PUB_KEY
Endpoint = $GW_ENDPOINT
AllowedIPs = 0.0.0.0/0
PersistentKeepalive = 25
EOF
    fi

    # 2. Kernel Parameters
    sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.all.route_localnet=1 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.all.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.default.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.wg0.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf."$DEFAULT_IFACE".rp_filter=2 >/dev/null 2>&1 || true

    # 3. Policy Routing Table 200 (Send player replies back via wg0 without affecting normal VPS internet)
    ip route flush table 200 2>/dev/null || true
    ip route add default dev wg0 table 200
    ip route add 10.200.0.0/24 dev wg0 table main 2>/dev/null || true

    # 4. Connection Tracking & Policy Rules
    iptables -t mangle -F PREROUTING 2>/dev/null || true
    iptables -t mangle -F OUTPUT 2>/dev/null || true

    iptables -t mangle -A PREROUTING -i wg0 -j CONNMARK --set-mark 0x1
    iptables -t mangle -A PREROUTING -m connmark --mark 0x1 -j CONNMARK --restore-mark
    iptables -t mangle -A OUTPUT -m connmark --mark 0x1 -j CONNMARK --restore-mark

    ip rule del fwmark 0x1 table 200 2>/dev/null || true
    ip rule add fwmark 0x1 table 200 priority 1000

    # 5. Direct Kernel DNAT to Docker Container IPs (100% bypasses docker-proxy!)
    iptables -t nat -F PREROUTING 2>/dev/null || true

    if command -v docker >/dev/null 2>&1; then
        echo "[+] Mapping active Docker containers directly..."
        for container in $(docker ps -q 2>/dev/null || true); do
            c_ip=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$container" 2>/dev/null || true)
            if [[ -n "$c_ip" ]]; then
                for c_port in $(docker inspect -f '{{range $p, $conf := .NetworkSettings.Ports}}{{$p}} {{end}}' "$container" 2>/dev/null | tr ' ' '\n' | awk -F/ '{print $1}' | sort -u || true); do
                    if [[ "$c_port" =~ ^[0-9]+$ && "$c_port" -ge 1024 && "$c_port" -le 65535 ]]; then
                        echo "  [+] Direct Bridge: wg0:${c_port} ──► ${c_ip}:${c_port} (Pure Real IP)"
                        iptables -t nat -A PREROUTING -i wg0 -p tcp --dport "$c_port" -j DNAT --to-destination "${c_ip}:${c_port}"
                        iptables -t nat -A PREROUTING -i wg0 -p udp --dport "$c_port" -j DNAT --to-destination "${c_ip}:${c_port}"
                    fi
                done
            fi
        done
    fi

    # Forwarding rules for tunnel and Docker bridges
    iptables -I INPUT 1 -i wg0 -j ACCEPT 2>/dev/null || true
    iptables -I FORWARD 1 -i wg0 -j ACCEPT 2>/dev/null || true
    iptables -I FORWARD 1 -o wg0 -j ACCEPT 2>/dev/null || true
    iptables -I DOCKER-USER 1 -j ACCEPT 2>/dev/null || true

    # Outbound MASQUERADE for container internet access only
    iptables -t nat -D POSTROUTING -s 172.16.0.0/12 ! -d 172.16.0.0/12 -j MASQUERADE 2>/dev/null || true
    iptables -t nat -A POSTROUTING -s 172.16.0.0/12 ! -d 172.16.0.0/12 -j MASQUERADE 2>/dev/null || true

    # Restart WireGuard
    systemctl restart wg-quick@wg0

    # Stop rinetd
    systemctl stop rinetd 2>/dev/null || true
    systemctl disable rinetd 2>/dev/null || true

    # Install and restart WireNet Watcher Daemon
    mkdir -p /opt/wirenet/scripts
    curl -fsSL -H "Cache-Control: no-cache" "https://raw.githubusercontent.com/UG88/wirenet/main/scripts/wirenet-watcher.sh?$(date +%s)" -o /opt/wirenet/scripts/wirenet-watcher.sh 2>/dev/null || true
    chmod 755 /opt/wirenet/scripts/wirenet-watcher.sh 2>/dev/null || true

    cat << 'EOF' > /etc/systemd/system/wirenet-watcher.service
[Unit]
Description=WireNet Dynamic Docker Port Watcher Daemon
After=docker.service wg-quick@wg0.service
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
    systemctl restart wirenet-watcher.service 2>/dev/null || true

    echo "=========================================================="
    echo " [✓] Pure Kernel Real-IP Engine is ACTIVE!"
    echo " ZERO plugins, ZERO server edits required."
    echo " Players will now show up with their genuine public IPs!"
    echo "=========================================================="
fi
