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

        # Find all port allocations for this container
        for port_mapping in $(docker port "$container" 2>/dev/null || true); do
            local host_port
            host_port=$(echo "$port_mapping" | awk -F: '{print $NF}' | tr -d ' ')
            local container_port
            container_port=$(echo "$port_mapping" | awk -F/ '{print $1}' | tr -d ' ')

            if [[ "$host_port" =~ ^[0-9]+$ ]]; then
                # Direct TCP Kernel DNAT to container IP (Bypasses docker-proxy & preserves Real Player IP!)
                iptables -t nat -D PREROUTING -i wg0 -p tcp --dport "$host_port" -j DNAT --to-destination "${c_ip}:${container_port}" 2>/dev/null || true
                iptables -t nat -I PREROUTING 1 -i wg0 -p tcp --dport "$host_port" -j DNAT --to-destination "${c_ip}:${container_port}"

                # Direct UDP Kernel DNAT
                iptables -t nat -D PREROUTING -i wg0 -p udp --dport "$host_port" -j DNAT --to-destination "${c_ip}:${container_port}" 2>/dev/null || true
                iptables -t nat -I PREROUTING 1 -i wg0 -p udp --dport "$host_port" -j DNAT --to-destination "${c_ip}:${container_port}"
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
