#!/usr/bin/env bash
# ==============================================================================
# WireNet 1-Command Installer & Control Center Launcher
# Developed by UG88 | https://github.com/UG88/wirenet
# ==============================================================================

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: WireNet installer must be run as root (or with sudo)." >&2
   exit 1
fi

curl -fsSL -H "Cache-Control: no-cache" "https://raw.githubusercontent.com/UG88/wirenet/main/wirenet.sh?$(date +%s)" | sudo bash
