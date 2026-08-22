#!/usr/bin/env bash
# ==============================================================================
# WireNet Firewall Controller (Zero-Downtime Dynamic Shield Manager)
# Developed by UG88 | https://github.com/UG88/wirenet
# Manage, toggle ON/OFF, and monitor TCP & UDP Minecraft Anti-DDoS Filters
# ==============================================================================

set -euo pipefail

DEFAULT_IFACE=$(ip route show default 2>/dev/null | awk '{print $5}' | head -n1)
DEFAULT_IFACE="${DEFAULT_IFACE:-eth0}"

function check_root() {
    if [[ $EUID -ne 0 ]]; then
        echo "[-] Error: This command must be run as root (or with sudo)." >&2
        exit 1
    fi
}

function enable_shield() {
    check_root
    local mode="${1:-normal}"
    echo "[+] Enabling WireNet Minecraft Anti-DDoS Shield (Mode: $mode)..."
    
    sysctl -w net.ipv4.tcp_syncookies=1 >/dev/null 2>&1 || true
    sysctl -w net.ipv4.tcp_max_syn_backlog=65536 >/dev/null 2>&1 || true

    iptables -N MC_TCP_FILTER 2>/dev/null || iptables -F MC_TCP_FILTER
    iptables -N MC_UDP_FILTER 2>/dev/null || iptables -F MC_UDP_FILTER

    iptables -A MC_TCP_FILTER -m state --state INVALID -j DROP
    iptables -A MC_TCP_FILTER -p tcp --tcp-flags ALL NONE -j DROP
    iptables -A MC_TCP_FILTER -p tcp --tcp-flags ALL ALL -j DROP
    iptables -A MC_TCP_FILTER -p tcp --tcp-flags SYN,FIN SYN,FIN -j DROP
    iptables -A MC_TCP_FILTER -p tcp --tcp-flags SYN,RST SYN,RST -j DROP

    if [[ "$mode" == "strict" ]]; then
        iptables -A MC_TCP_FILTER -p tcp --syn -m hashlimit --hashlimit-above 10/sec --hashlimit-burst 20 --hashlimit-mode srcip --hashlimit-name mc_tcp_limit -j DROP
        iptables -A MC_UDP_FILTER -p udp -m hashlimit --hashlimit-above 40/sec --hashlimit-burst 80 --hashlimit-mode srcip --hashlimit-name mc_udp_limit -j DROP
    else
        iptables -A MC_TCP_FILTER -p tcp --syn -m hashlimit --hashlimit-above 25/sec --hashlimit-burst 50 --hashlimit-mode srcip --hashlimit-name mc_tcp_limit -j DROP
        iptables -A MC_UDP_FILTER -p udp -m hashlimit --hashlimit-above 60/sec --hashlimit-burst 120 --hashlimit-mode srcip --hashlimit-name mc_udp_limit -j DROP
    fi
    iptables -A MC_TCP_FILTER -j ACCEPT
    iptables -A MC_UDP_FILTER -j ACCEPT

    iptables -D INPUT -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25600,30000:40000 -j MC_TCP_FILTER 2>/dev/null || true
    iptables -A INPUT -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25600,30000:40000 -j MC_TCP_FILTER

    iptables -D INPUT -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25600,30000:40000 -j MC_UDP_FILTER 2>/dev/null || true
    iptables -A INPUT -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25600,30000:40000 -j MC_UDP_FILTER

    echo "[✓] WireNet Firewall Shield is now ENABLED ($mode mode). Existing players remain connected!"
}

function disable_shield() {
    check_root
    echo "[+] Disabling WireNet Firewall Shield..."
    iptables -D INPUT -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25600,30000:40000 -j MC_TCP_FILTER 2>/dev/null || true
    iptables -D INPUT -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25600,30000:40000 -j MC_UDP_FILTER 2>/dev/null || true
    iptables -F MC_TCP_FILTER 2>/dev/null || true
    iptables -F MC_UDP_FILTER 2>/dev/null || true
    echo "[✓] WireNet Firewall Shield is now DISABLED. All traffic passes directly."
}

function show_status() {
    echo "=========================================================="
    echo " WireNet Minecraft Firewall & Anti-DDoS Status"
    echo "=========================================================="
    if iptables -L INPUT -n 2>/dev/null | grep -q "MC_TCP_FILTER"; then
        echo " Shield Status: [ ACTIVE / PROTECTED ]"
        echo ""
        echo " Live Filter Statistics (Dropped Packets & Traffic):"
        iptables -L MC_TCP_FILTER -n -v --line-numbers 2>/dev/null
        echo ""
        iptables -L MC_UDP_FILTER -n -v --line-numbers 2>/dev/null
    else
        echo " Shield Status: [ DISABLED / PASS-THROUGH ]"
    fi
    echo "=========================================================="
}

function show_live_status() {
    trap 'break' INT
    while true; do
        clear
        echo "================================================================================"
        echo " 🛡️  WireNet Live Real-Time Anti-DDoS Attack & Packet Monitor (Refreshing 1s)   "
        echo "                   [ Press 'q' or Ctrl+C to return to menu ]                    "
        echo "================================================================================"
        if iptables -L INPUT -n 2>/dev/null | grep -q "MC_TCP_FILTER"; then
            echo " Shield Status: [ ACTIVE / PROTECTED ]"
            echo ""
            echo "--- TCP Filter Statistics (SYN Cookies, Malformed Drops, Rate Limits) ---"
            iptables -L MC_TCP_FILTER -n -v --line-numbers 2>/dev/null || true
            echo ""
            echo "--- UDP Filter Statistics (Bedrock Reflection & Flood Drops) ---"
            iptables -L MC_UDP_FILTER -n -v --line-numbers 2>/dev/null || true
        else
            echo " Shield Status: [ DISABLED / PASS-THROUGH ]"
        fi
        echo "================================================================================"

        KEY=""
        if [[ -e /dev/tty ]]; then
            read -rsn1 -t 1 KEY </dev/tty 2>/dev/null || true
        else
            read -rsn1 -t 1 KEY 2>/dev/null || true
        fi
        if [[ "${KEY:-}" =~ ^[qQ]$ ]]; then
            break
        fi
    done
    trap - INT
}

case "${1:-status}" in
    enable|on)
        enable_shield normal
        ;;
    strict)
        enable_shield strict
        ;;
    disable|off)
        disable_shield
        ;;
    status)
        show_status
        ;;
    live|watch)
        show_live_status
        ;;
    *)
        echo "Usage: sudo $0 {enable|disable|strict|status|live}"
        exit 1
        ;;
esac
