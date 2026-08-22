#!/usr/bin/env bash
# ==============================================================================
# WireNet Universal Auto-Troubleshooter (Smart Auto-Detection)
# Runs full diagnostics and self-healing fixes on either Gateway or Node
# ==============================================================================

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

# Detect role
if [[ -f /etc/wireguard/gateway_private.key ]] || ip addr show dev wg0 2>/dev/null | grep -q "10.200.0.1/"; then
    echo "[+] Detected Role: WireNet GATEWAY VPS"
    curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/troubleshoot-gateway.sh | sudo bash
else
    echo "[+] Detected Role: WireNet PTERODACTYL NODE VPS"
    curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/troubleshoot-node.sh | sudo bash
fi
