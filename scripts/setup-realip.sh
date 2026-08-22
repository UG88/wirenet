#!/usr/bin/env bash
# ==============================================================================
# WireNet 100% Native Kernel Real-IP Engine (Direct Container IP Routing)
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

DEFAULT_IFACE=$(ip route show default 2>/dev/null | awk '{print $5}' | head -n1)
DEFAULT_IFACE="${DEFAULT_IFACE:-eth0}"

echo "=========================================================="
echo " WireNet Native Real-IP Engine: ${ROLE}"
echo " (Direct Kernel Routing -> 100% Genuine Player IPs)"
echo "=========================================================="

if [[ "$IS_GATEWAY" == true ]]; then
    echo "[+] Configuring Gateway for Transparent Real-IP Delivery..."
    
    # 1. Enable Kernel IP Forwarding
    sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true

    # 2. Stop HAProxy if running
    systemctl stop haproxy 2>/dev/null || true
    systemctl disable haproxy 2>/dev/null || true

    # 3. Transparent DNAT without Masquerade on game ports (Preserves Player Source IP!)
    iptables -t nat -D PREROUTING -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination 10.200.0.2 2>/dev/null || true
    iptables -t nat -A PREROUTING -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination 10.200.0.2

    iptables -t nat -D PREROUTING -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DNAT --to-destination 10.200.0.2 2>/dev/null || true
    iptables -t nat -A PREROUTING -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DNAT --to-destination 10.200.0.2

    # Allow Forwarding
    iptables -A FORWARD -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT 2>/dev/null || true
    iptables -A FORWARD -i "$DEFAULT_IFACE" -o wg0 -j ACCEPT 2>/dev/null || true
    iptables -A FORWARD -i wg0 -o "$DEFAULT_IFACE" -j ACCEPT 2>/dev/null || true

    # Remove any MASQUERADE on wg0 so player source IP is 100% untouched!
    iptables -t nat -D POSTROUTING -o wg0 -j MASQUERADE 2>/dev/null || true

    # Outbound NAT only for Node traffic reaching internet via Gateway
    iptables -t nat -D POSTROUTING -o "$DEFAULT_IFACE" -j MASQUERADE 2>/dev/null || true
    iptables -t nat -A POSTROUTING -o "$DEFAULT_IFACE" -j MASQUERADE

    echo "=========================================================="
    echo " [✓] Gateway is passing raw, untouched player IPs to Node!"
    echo "=========================================================="

else
    echo "[+] Configuring Node Kernel, WireGuard & Direct Container Routing..."

    # 1. Update WireGuard AllowedIPs on Node so it can route return packets back to any player IP
    GW_KEY=$(wg show wg0 peers 2>/dev/null | head -n1 || true)
    if [[ -n "$GW_KEY" ]]; then
        echo "[+] Updating WireGuard peer routing to allow player return traffic..."
        wg set wg0 peer "$GW_KEY" allowed-ips 0.0.0.0/0 2>/dev/null || true
    fi

    if [[ -f /etc/wireguard/wg0.conf ]]; then
        sed -i 's/AllowedIPs = .*/AllowedIPs = 0.0.0.0\/0/g' /etc/wireguard/wg0.conf 2>/dev/null || true
    fi

    # 2. Enable Kernel IP Forwarding & Loose Reverse-Path Filtering
    sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.all.route_localnet=1 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.all.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.default.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.wg0.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf."$DEFAULT_IFACE".rp_filter=2 >/dev/null 2>&1 || true

    # 3. Connection Tracking & Policy Routing (Send game replies back via wg0)
    iptables -t mangle -F PREROUTING 2>/dev/null || true
    iptables -t mangle -F OUTPUT 2>/dev/null || true

    iptables -t mangle -A PREROUTING -i wg0 -j CONNMARK --set-mark 0x1
    iptables -t mangle -A PREROUTING -m connmark --mark 0x1 -j CONNMARK --restore-mark
    iptables -t mangle -A OUTPUT -m connmark --mark 0x1 -j CONNMARK --restore-mark

    # Route marked packets back via wg0 to the Gateway using routing table 100
    ip rule del fwmark 0x1 table 100 2>/dev/null || true
    ip rule add fwmark 0x1 table 100 priority 1000
    ip route flush table 100 2>/dev/null || true
    ip route add default dev wg0 table 100

    # 4. PREVENT Docker from masquerading packets coming from wg0!
    iptables -t nat -D POSTROUTING -m connmark --mark 0x1 -j ACCEPT 2>/dev/null || true
    iptables -t nat -I POSTROUTING 1 -m connmark --mark 0x1 -j ACCEPT

    # 5. Direct Kernel DNAT to Docker Container IPs (100% bypasses docker-proxy!)
    if command -v docker >/dev/null 2>&1; then
        echo "[+] Mapping active Docker containers directly..."
        for container in $(docker ps -q 2>/dev/null || true); do
            c_ip=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$container" 2>/dev/null || true)
            if [[ -n "$c_ip" ]]; then
                for port_mapping in $(docker port "$container" 2>/dev/null || true); do
                    h_port=$(echo "$port_mapping" | awk -F: '{print $NF}' | tr -d ' ')
                    c_port=$(echo "$port_mapping" | awk -F/ '{print $1}' | tr -d ' ')
                    if [[ "$h_port" =~ ^[0-9]+$ ]]; then
                        echo "  [+] Direct Bridge: wg0:${h_port} ──► ${c_ip}:${c_port} (Pure Real IP)"
                        iptables -t nat -D PREROUTING -i wg0 -p tcp --dport "$h_port" -j DNAT --to-destination "${c_ip}:${c_port}" 2>/dev/null || true
                        iptables -t nat -I PREROUTING 1 -i wg0 -p tcp --dport "$h_port" -j DNAT --to-destination "${c_ip}:${c_port}"

                        iptables -t nat -D PREROUTING -i wg0 -p udp --dport "$h_port" -j DNAT --to-destination "${c_ip}:${c_port}" 2>/dev/null || true
                        iptables -t nat -I PREROUTING 1 -i wg0 -p udp --dport "$h_port" -j DNAT --to-destination "${c_ip}:${c_port}"
                    fi
                done
            fi
        done
    fi

    # Forwarding rules for tunnel and Docker bridges
    iptables -I INPUT 1 -i wg0 -j ACCEPT 2>/dev/null || true
    iptables -I FORWARD 1 -i wg0 -j ACCEPT 2>/dev/null || true
    iptables -I FORWARD 1 -o wg0 -j ACCEPT 2>/dev/null || true

    # Outbound MASQUERADE for container internet access only
    iptables -t nat -D POSTROUTING -s 172.16.0.0/12 ! -d 172.16.0.0/12 -j MASQUERADE 2>/dev/null || true
    iptables -t nat -A POSTROUTING -s 172.16.0.0/12 ! -d 172.16.0.0/12 -j MASQUERADE 2>/dev/null || true

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

    # Stop rinetd
    systemctl stop rinetd 2>/dev/null || true
    systemctl disable rinetd 2>/dev/null || true

    echo "=========================================================="
    echo " [✓] Direct Container Real-IP Routing is ACTIVE!"
    echo " All incoming packets go directly into container sockets."
    echo " Players will now show up with their genuine public IPs!"
    echo "=========================================================="
fi
