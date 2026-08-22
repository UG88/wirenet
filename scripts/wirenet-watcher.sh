#!/usr/bin/env bash
# ==============================================================================
# WireNet Dynamic Port Watcher (Automated Docker Port Bridge Daemon)
# Developed by UG88 | https://github.com/UG88/wirenet
# Automatically detects any newly created Docker / Pterodactyl server and bridges it
# ==============================================================================

set -euo pipefail

NODE_IP=$(ip -4 addr show dev wg0 2>/dev/null | grep "inet " | awk '{print $2}' | cut -d/ -f1 || echo "10.200.0.2")
PRIMARY_IP=$(curl -s -4 ifconfig.me 2>/dev/null || curl -s -4 icanhazip.com 2>/dev/null || ip route get 1.1.1.1 2>/dev/null | awk '{print $7; exit}' || echo "127.0.0.1")

sync_ports() {
    local changed=false
    local current_ports
    current_ports=$(ss -tulpn 2>/dev/null | grep -E "dockerd|java|wings" | awk '{print $5}' | awk -F: '{print $NF}' | sort -u || true)

    for port in $current_ports; do
        if [[ "$port" =~ ^[0-9]+$ && "$port" -ge 1024 && "$port" -le 65535 ]]; then
            if ! grep -q "$NODE_IP $port " /etc/rinetd.conf 2>/dev/null; then
                echo "$NODE_IP $port $PRIMARY_IP $port" >> /etc/rinetd.conf
                changed=true
                echo "[WireNet Watcher] Dynamically bridged new game server port: ${port}"
            fi
        fi
    done

    if [[ "$changed" == true ]]; then
        systemctl restart rinetd 2>/dev/null || true
    fi
}

echo "[WireNet Watcher] Service active. Listening for new Pterodactyl Docker containers..."

# Initial port sync
sync_ports

# Event loop: Watch docker container start/die events and sync ports automatically
if command -v docker >/dev/null 2>&1; then
    docker events --filter 'event=start' --filter 'event=die' 2>/dev/null | while read -r event; do
        sleep 2
        sync_ports
    done
else
    # Polling fallback if docker CLI is not present
    while true; do
        sleep 10
        sync_ports
    done
fi
