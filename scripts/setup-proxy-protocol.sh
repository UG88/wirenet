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

# Minecraft Standard Port (PROXY Protocol v2 enabled)
listen minecraft_25565
    bind 0.0.0.0:25565
    mode tcp
    server node1 ${NODE_IP}:25565 send-proxy-v2

# Additional Game Ports
listen minecraft_25566
    bind 0.0.0.0:25566
    mode tcp
    server node1 ${NODE_IP}:25566 send-proxy-v2

listen minecraft_25567
    bind 0.0.0.0:25567
    mode tcp
    server node1 ${NODE_IP}:25567 send-proxy-v2
EOF

    # Restart HAProxy
    systemctl enable haproxy 2>/dev/null || true
    systemctl restart haproxy

    echo ""
    echo "=========================================================="
    echo " [✓] HAProxy PROXY Protocol v2 is ACTIVE on Gateway!"
    echo "=========================================================="
    echo ""
    echo "To display Real Player IPs in Minecraft:"
    echo " 1. For Paper / Purpur (1.19+):"
    echo "    In config/paper-global.yml, set:"
    echo "      proxies:"
    echo "        proxy-protocol: true"
    echo ""
    echo " 2. Or for any Paper/Spigot server:"
    echo "    Drop 'ProxyProtocol.jar' into your server's plugins/ folder:"
    echo "    https://github.com/LOOHP/ProxyProtocol/releases"
    echo ""
    echo " 3. For Velocity / BungeeCord:"
    echo "    In velocity.toml / config.yml, set:"
    echo "      proxy-protocol = true"
    echo "=========================================================="

elif [[ "$ACTION" == "disable" ]]; then
    echo "[+] Disabling HAProxy..."
    systemctl stop haproxy 2>/dev/null || true
    systemctl disable haproxy 2>/dev/null || true
    echo "[✓] HAProxy disabled. Direct NAT mode restored."
fi
