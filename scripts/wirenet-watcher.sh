#!/usr/bin/env bash
# ==============================================================================
# WireNet Dynamic Container Port Watcher Daemon (Direct Container IP Bridge)
# Developed by UG88 | https://github.com/UG88/wirenet
# Automatically DNATs directly to Docker container IPs for 100% Real Player IPs
# ==============================================================================

set -euo pipefail

sync_container_routes() {
    if ! command -v docker >/dev/null 2>&1; then
        return
    fi

    # Scan running Docker containers and map directly to container internal IPs
    for container in $(docker ps -q 2>/dev/null || true); do
        local c_ip
        c_ip=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$container" 2>/dev/null || true)
        
        if [[ -z "$c_ip" ]]; then
            continue
        fi

        # Find all exposed container ports using docker inspect
        local ports
        ports=$(docker inspect -f '{{range $p, $conf := .NetworkSettings.Ports}}{{$p}} {{end}}' "$container" 2>/dev/null | tr ' ' '\n' | awk -F/ '{print $1}' | sort -u || true)

        for c_port in $ports; do
            if [[ "$c_port" =~ ^[0-9]+$ && "$c_port" -ge 1024 && "$c_port" -le 65535 ]]; then
                # Direct TCP Kernel DNAT to container IP (Bypasses docker-proxy & preserves Real Player IP!)
                iptables -t nat -D PREROUTING -i wg0 -p tcp --dport "$c_port" -j DNAT --to-destination "${c_ip}:${c_port}" 2>/dev/null || true
                iptables -t nat -I PREROUTING 1 -i wg0 -p tcp --dport "$c_port" -j DNAT --to-destination "${c_ip}:${c_port}"

                # Direct UDP Kernel DNAT
                iptables -t nat -D PREROUTING -i wg0 -p udp --dport "$c_port" -j DNAT --to-destination "${c_ip}:${c_port}" 2>/dev/null || true
                iptables -t nat -I PREROUTING 1 -i wg0 -p udp --dport "$c_port" -j DNAT --to-destination "${c_ip}:${c_port}"
            fi
        done
    done
}

echo "[WireNet Watcher] Active. Mapping direct container IPs for 100% Real Player IPs..."

# Initial sync
sync_container_routes

# Event listener for container start/die events
if command -v docker >/dev/null 2>&1; then
    docker events --filter 'event=start' --filter 'event=die' 2>/dev/null | while read -r event; do
        sleep 1
        sync_container_routes
    done
else
    while true; do
        sleep 5
        sync_container_routes
    done
fi
