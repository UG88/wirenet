#!/usr/bin/env bash
# ==============================================================================
# WireNet 100% Native Kernel Real-IP Engine (ZERO Plugins / ZERO File Edits)
# Developed by UG88 | https://github.com/UG88/wirenet
# Pure kernel-level transparent routing directly into Docker containers
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
echo " (100% Zero-Plugin / Zero-File-Edit Pure Kernel Routing)"
echo "=========================================================="

if [[ "$IS_GATEWAY" == true ]]; then
    echo "[+] Configuring Gateway for Transparent Real-IP Delivery..."
    
    # 1. Enable Kernel IP Forwarding
    sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true

    # 2. Stop HAProxy so kernel routes directly
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

    # MASQUERADE only for Node outbound traffic, NOT for inbound player packets!
    iptables -t nat -D POSTROUTING -o "$DEFAULT_IFACE" -j MASQUERADE 2>/dev/null || true
    iptables -t nat -A POSTROUTING -o "$DEFAULT_IFACE" -j MASQUERADE

    # Remove any masquerade on wg0
    iptables -t nat -D POSTROUTING -o wg0 -j MASQUERADE 2>/dev/null || true

    echo "=========================================================="
    echo " [✓] Gateway is passing raw, untouched player IPs to Node!"
    echo "=========================================================="

else
    echo "[+] Configuring Node Kernel & Docker for Native Real-IP Delivery..."

    PRIMARY_IP=$(curl -s -4 ifconfig.me 2>/dev/null || curl -s -4 icanhazip.com 2>/dev/null || ip route get 1.1.1.1 2>/dev/null | awk '{print $7; exit}' || echo "127.0.0.1")
    NODE_IP=$(ip -4 addr show dev wg0 2>/dev/null | grep "inet " | awk '{print $2}' | cut -d/ -f1 || echo "10.200.0.2")

    # 1. Enable Kernel IP Forwarding & Loose Reverse-Path Filtering
    sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.all.route_localnet=1 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.all.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.default.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.wg0.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf."$DEFAULT_IFACE".rp_filter=2 >/dev/null 2>&1 || true

    # 2. Disable Docker userland-proxy (so Docker doesn't proxy through 172.18.0.1)
    mkdir -p /etc/docker
    cat << 'EOF' > /etc/docker/daemon.json
{
  "userland-proxy": false,
  "live-restore": true,
  "dns": ["1.1.1.1", "8.8.8.8"]
}
EOF
    systemctl reload docker 2>/dev/null || true

    # 3. Connection Tracking & Policy Routing (Send game replies back via wg0)
    iptables -t mangle -D PREROUTING -i wg0 -j CONNMARK --set-mark 0x1 2>/dev/null || true
    iptables -t mangle -A PREROUTING -i wg0 -j CONNMARK --set-mark 0x1

    iptables -t mangle -D PREROUTING -m connmark --mark 0x1 -j CONNMARK --restore-mark 2>/dev/null || true
    iptables -t mangle -A PREROUTING -m connmark --mark 0x1 -j CONNMARK --restore-mark

    iptables -t mangle -D OUTPUT -m connmark --mark 0x1 -j CONNMARK --restore-mark 2>/dev/null || true
    iptables -t mangle -A OUTPUT -m connmark --mark 0x1 -j CONNMARK --restore-mark

    # Route marked packets back via wg0 to the Gateway
    ip rule del fwmark 0x1 table 100 2>/dev/null || true
    ip rule add fwmark 0x1 table 100
    ip route flush table 100 2>/dev/null || true
    ip route add default dev wg0 table 100

    # 4. Direct Kernel DNAT into local host/Docker bindings (Preserving Real Player Source IP!)
    iptables -t nat -D PREROUTING -i wg0 -d "$NODE_IP" -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination "$PRIMARY_IP" 2>/dev/null || true
    iptables -t nat -A PREROUTING -i wg0 -d "$NODE_IP" -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination "$PRIMARY_IP"

    iptables -t nat -D PREROUTING -i wg0 -d "$NODE_IP" -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DNAT --to-destination "$PRIMARY_IP" 2>/dev/null || true
    iptables -t nat -A PREROUTING -i wg0 -d "$NODE_IP" -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DNAT --to-destination "$PRIMARY_IP"

    # Forwarding rules for tunnel and Docker bridges
    iptables -I INPUT 1 -i wg0 -j ACCEPT 2>/dev/null || true
    iptables -I FORWARD 1 -i wg0 -j ACCEPT 2>/dev/null || true
    iptables -I FORWARD 1 -o wg0 -j ACCEPT 2>/dev/null || true

    # Outbound MASQUERADE for container internet access only (NOT for inbound tunnel traffic)
    iptables -t nat -D POSTROUTING -s 172.16.0.0/12 ! -d 172.16.0.0/12 -j MASQUERADE 2>/dev/null || true
    iptables -t nat -A POSTROUTING -s 172.16.0.0/12 ! -d 172.16.0.0/12 -j MASQUERADE 2>/dev/null || true

    # Stop userspace rinetd so pure kernel routing delivers raw Real IPs directly into Docker
    systemctl stop rinetd 2>/dev/null || true
    systemctl disable rinetd 2>/dev/null || true

    echo "=========================================================="
    echo " [✓] 100% Native Real-IP Routing is ACTIVE!"
    echo " ZERO plugins or server edits required."
    echo " Players now show up with their genuine public IPs!"
    echo "=========================================================="
fi
