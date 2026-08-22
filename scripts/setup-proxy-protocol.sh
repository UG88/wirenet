#!/usr/bin/env bash
# ==============================================================================
# WireNet PROXY Protocol v2 & Real Player IP Ingress Setup
# Developed by UG88 | https://github.com/UG88/wirenet
# Passes 100% Genuine Player Public IPs to Minecraft servers (Paper/Velocity/Bungee/Fabric)
# ==============================================================================

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

ACTION="${1:-enable}"
NODE_IP="${2:-10.200.0.2}"

echo "=========================================================="
echo " WireNet PROXY Protocol v2 Real Player IP Manager"
echo "=========================================================="

if [[ "$ACTION" == "enable" ]]; then
    echo "[+] Installing and configuring HAProxy for PROXY Protocol v2..."
    apt-get update -qq && apt-get install -y haproxy

    # 1. Clear iptables TCP DNAT on Gateway so HAProxy handles the ports directly
    DEFAULT_IFACE=$(ip route show default 2>/dev/null | awk '{print $5}' | head -n1 || echo "eth0")
    iptables -t nat -D PREROUTING -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination "$NODE_IP" 2>/dev/null || true
    iptables -t nat -D PREROUTING -p tcp -m multiport --dports 25565:25600,30000:40000 -j DNAT --to-destination "$NODE_IP" 2>/dev/null || true

    # 2. Build HAProxy config for Minecraft ports (25565 - 25700)
    cat << EOF > /etc/haproxy/haproxy.cfg
global
    log /dev/log local0
    maxconn 20000
    user haproxy
    group haproxy
    daemon

defaults
    log     global
    mode    tcp
    option  tcplog
    option  dontlognull
    timeout connect 5s
    timeout client  30m
    timeout server  30m
EOF

    for port in $(seq 25565 25700); do
        cat << EOF >> /etc/haproxy/haproxy.cfg

listen minecraft_${port}
    bind 0.0.0.0:${port}
    mode tcp
    server node1 ${NODE_IP}:${port} send-proxy-v2
EOF
    done

    # 3. Stop any conflicting background forwarders and start HAProxy
    systemctl stop rinetd wirenet-gateway 2>/dev/null || true
    systemctl disable rinetd 2>/dev/null || true
    systemctl enable --now haproxy 2>/dev/null || true
    systemctl restart haproxy

    echo ""
    echo "=========================================================="
    echo " [✓] HAProxy PROXY Protocol v2 is ACTIVE on Gateway!"
    echo " Ports 25565-25700 are broadcasting PROXY v2 headers."
    echo "=========================================================="
    echo ""
    echo "To display Real Player IPs in Minecraft:"
    echo " 1. For Paper / Purpur (1.19+):"
    echo "    In config/paper-global.yml, set:"
    echo "      proxies:"
    echo "        proxy-protocol: true"
    echo ""
    echo " 2. Or for any Spigot/Paper/Fabric server:"
    echo "    Drop 'ProxyProtocol.jar' into your server's plugins/ folder:"
    echo "    https://github.com/LOOHP/ProxyProtocol/releases"
    echo ""
    echo " 3. For Velocity / BungeeCord:"
    echo "    In velocity.toml / config.yml, set:"
    echo "      proxy-protocol = true"
    echo "=========================================================="

elif [[ "$ACTION" == "disable" ]]; then
    echo "[+] Disabling HAProxy and restoring standard NAT..."
    systemctl stop haproxy 2>/dev/null || true
    systemctl disable haproxy 2>/dev/null || true

    # Restore standard kernel DNAT
    DEFAULT_IFACE=$(ip route show default 2>/dev/null | awk '{print $5}' | head -n1 || echo "eth0")
    iptables -t nat -A PREROUTING -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination "$NODE_IP" 2>/dev/null || true
    echo "[✓] HAProxy disabled. Direct NAT mode restored."
fi
