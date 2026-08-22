#!/usr/bin/env bash
# ==============================================================================
# WireNet Pure Kernel Real-IP & Policy Routing Setup
# Developed by UG88 | https://github.com/UG88/wirenet
# Preserves 100% Genuine Player IPs in Minecraft (Zero Plugins / Zero IP Collisions)
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
echo " WireNet Pure Kernel Real-IP Ingress Setup: ${ROLE}"
echo "=========================================================="

if [[ "$IS_GATEWAY" == true ]]; then
    echo "[+] Configuring Gateway for Transparent Real-IP Delivery..."
    
    # 1. Enable Kernel IP Forwarding
    sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true

    # 2. Stop HAProxy if running so kernel takes over port 25565
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

    echo "=========================================================="
    echo " [✓] Gateway is now sending 100% Real Player IPs to Node!"
    echo "=========================================================="

else
    echo "[+] Configuring Node for Policy-Based Asymmetric Return Routing..."

    PRIMARY_IP=$(curl -s -4 ifconfig.me 2>/dev/null || curl -s -4 icanhazip.com 2>/dev/null || ip route get 1.1.1.1 2>/dev/null | awk '{print $7; exit}' || echo "127.0.0.1")
    NODE_IP=$(ip -4 addr show dev wg0 2>/dev/null | grep "inet " | awk '{print $2}' | cut -d/ -f1 || echo "10.200.0.2")

    # 1. Enable Kernel IP Forwarding & Loose Reverse-Path Filtering
    sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.all.route_localnet=1 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.all.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.default.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf.wg0.rp_filter=2 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.conf."$DEFAULT_IFACE".rp_filter=2 >/dev/null 2>&1 || true

    # 2. Connection Tracking & Policy Routing
    # Mark all incoming connections from wg0
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

    # 3. Direct Kernel DNAT into local Docker bindings (Preserving Real Player Source IP!)
    iptables -t nat -D PREROUTING -i wg0 -d "$NODE_IP" -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination "$PRIMARY_IP" 2>/dev/null || true
    iptables -t nat -A PREROUTING -i wg0 -d "$NODE_IP" -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination "$PRIMARY_IP"

    iptables -t nat -D PREROUTING -i wg0 -d "$NODE_IP" -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DNAT --to-destination "$PRIMARY_IP" 2>/dev/null || true
    iptables -t nat -A PREROUTING -i wg0 -d "$NODE_IP" -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DNAT --to-destination "$PRIMARY_IP"

    # Forwarding rules
    iptables -I INPUT 1 -i wg0 -j ACCEPT 2>/dev/null || true
    iptables -I FORWARD 1 -i wg0 -j ACCEPT 2>/dev/null || true
    iptables -I FORWARD 1 -o wg0 -j ACCEPT 2>/dev/null || true

    # Outbound MASQUERADE for Docker internet access
    iptables -t nat -D POSTROUTING -s 172.16.0.0/12 -j MASQUERADE 2>/dev/null || true
    iptables -t nat -A POSTROUTING -s 172.16.0.0/12 -j MASQUERADE 2>/dev/null || true

    # Stop rinetd so pure kernel routing delivers raw Real IPs directly
    systemctl stop rinetd 2>/dev/null || true
    systemctl disable rinetd 2>/dev/null || true

    echo "=========================================================="
    echo " [✓] Node Pure Kernel Real-IP Routing is ACTIVE!"
    echo " Minecraft console will now log 100% Genuine Player IPs!"
    echo " Zero plugins required. /ban-ip will ban only that player."
    echo "=========================================================="
fi
