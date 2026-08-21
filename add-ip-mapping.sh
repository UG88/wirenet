#!/usr/bin/env bash
# ==============================================================================
# WireNet Multi-IP Mapper (Map Dedicated Public IPs to Backend Nodes)
# Allows multiple Pterodactyl Nodes (VMs) to use the SAME default port (e.g. 25565)
# ==============================================================================

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

PUBLIC_IP="${1:-}"
NODE_IP="${2:-}"
PORT_RANGE="${3:-25565:25600,30000:40000}"

if [[ -z "$PUBLIC_IP" || -z "$NODE_IP" ]]; then
    echo "=========================================================="
    echo " WireNet Multi-IP Port Mapper"
    echo "=========================================================="
    echo " Usage: sudo $0 <GATEWAY_PUBLIC_IP> <NODE_TUNNEL_IP> [PORT_RANGE]"
    echo ""
    echo " Example:"
    echo "   sudo $0 3.108.50.21 10.200.0.2"
    echo "   sudo $0 3.108.50.22 10.200.0.3"
    echo "   sudo $0 3.108.50.23 10.200.0.4"
    echo "=========================================================="
    exit 1
fi

echo "[+] Mapping Public IP '$PUBLIC_IP' -> Node Tunnel IP '$NODE_IP' (Ports: $PORT_RANGE)..."

# Apply live iptables DNAT rules
iptables -t nat -A PREROUTING -d "$PUBLIC_IP" -p tcp -m multiport --dports "$PORT_RANGE" -j DNAT --to-destination "$NODE_IP"
iptables -t nat -A PREROUTING -d "$PUBLIC_IP" -p udp -m multiport --dports "$PORT_RANGE" -j DNAT --to-destination "$NODE_IP"

# Append rules to /etc/wireguard/wg0.conf for persistence across reboots
if [[ -f /etc/wireguard/wg0.conf ]]; then
    sed -i "/^PrivateKey/a # Mapping for $PUBLIC_IP -> $NODE_IP\nPostUp = iptables -t nat -A PREROUTING -d $PUBLIC_IP -p tcp -m multiport --dports $PORT_RANGE -j DNAT --to-destination $NODE_IP\nPostUp = iptables -t nat -A PREROUTING -d $PUBLIC_IP -p udp -m multiport --dports $PORT_RANGE -j DNAT --to-destination $NODE_IP\nPostDown = iptables -t nat -D PREROUTING -d $PUBLIC_IP -p tcp -m multiport --dports $PORT_RANGE -j DNAT --to-destination $NODE_IP\nPostDown = iptables -t nat -D PREROUTING -d $PUBLIC_IP -p udp -m multiport --dports $PORT_RANGE -j DNAT --to-destination $NODE_IP" /etc/wireguard/wg0.conf
fi

echo "=========================================================="
echo " [✓] Mapping applied successfully!"
echo " All traffic arriving at $PUBLIC_IP (Ports $PORT_RANGE)"
echo " will now seamlessly route to Node $NODE_IP!"
echo " Both Node 1 and Node 2 can now run on port 25565!"
echo "=========================================================="
