#!/usr/bin/env bash
# ==============================================================================
# WireNet Interactive Control Center & Master Manager
# Developed by UG88 | https://github.com/UG88/wirenet
# Kernel-Level Ingress & Anti-DDoS Shield for Pterodactyl Game Servers
# ==============================================================================

set -euo pipefail

# ANSI Colors & Styling
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
WHITE='\033[1;37m'
BOLD='\033[1m'
DIM='\033[2m'
REV='\033[7m'
NC='\033[0m'

if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}[-] Error: WireNet Manager must be run as root (or with sudo).${NC}" >&2
   exit 1
fi

REPO_BASE="https://raw.githubusercontent.com/UG88/wirenet/main/scripts"

# Helper to fetch fresh scripts bypassing GitHub CDN cache
fetch_script() {
    local script_name="$1"
    shift
    curl -fsSL -H "Cache-Control: no-cache" "${REPO_BASE}/${script_name}?$(date +%s)" | sudo bash -s -- "$@"
}

# Header with fixed ASCII Art and UG88 Author branding
draw_header() {
    clear
    echo -e "${CYAN}${BOLD}"
    cat << 'EOF'
================================================================================
  __          ___          _   _      _   
  \ \        / (_)        | \ | |    | |  
   \ \  /\  / / _ _ __ ___|  \| | ___| |_ 
    \ \/  \/ / | | '__/ _ \ . ` |/ _ \ __|
     \  /\  /  | | | |  __/ |\  |  __/ |_ 
      \/  \/   |_|_|  \___|_| \_|\___|\__|

     Kernel-Level Minecraft Ingress & Anti-DDoS Shield for Pterodactyl
                Developed by UG88 | GitHub: UG88/wirenet
================================================================================
EOF
    echo -e "${NC}"
}

# Helper function to print an info/guide box
show_guide() {
    local title="$1"
    local desc="$2"
    local example="$3"
    echo -e "\n${YELLOW}${BOLD}┌── ℹ️  ${title} ──────────────────────────────────────────${NC}"
    echo -e "${YELLOW}│${NC} ${desc}"
    if [[ -n "$example" ]]; then
        echo -e "${YELLOW}│${NC} ${CYAN}${BOLD}Example:${NC} ${example}"
    fi
    echo -e "${YELLOW}└───────────────────────────────────────────────────────────────────${NC}\n"
}

# Submenu: Status & Telemetry Dashboard
status_menu() {
    local STAT_CHOICE=0
    local STAT_OPTIONS=(
        "View Complete Telemetry Dashboard (All Metrics at Once)"
        "Check WireGuard Interface & Handshake Telemetry Only"
        "Check Live Tunnel Latency & Ping to Nodes Only"
        "Inspect Active Minecraft & Game Server Ports Only"
        "Inspect Anti-DDoS Attack Drop Counters Only"
        "Back to Main Menu"
    )

    while true; do
        draw_header
        echo -e "${YELLOW}${BOLD}  📊  LIVE STATUS & TELEMETRY SUBMENU (by UG88)${NC}"
        show_guide "Telemetry Options Guide" \
                   "Choose whether to view all telemetry at once or inspect individual network subsystems." \
                   "Sub-millisecond ping, handshakes, and port bindings are tested in real time."

        echo -e "  Use ${BOLD}UP/DOWN Arrow Keys${NC} and press ${BOLD}ENTER${NC} (or type a number):\n"

        for i in "${!STAT_OPTIONS[@]}"; do
            if [[ $i -eq $STAT_CHOICE ]]; then
                echo -e "  ${CYAN}${BOLD}▶ ${REV} [ $((i + 1)) ] ${STAT_OPTIONS[$i]} ${NC}"
            else
                echo -e "    [ $((i + 1)) ] ${STAT_OPTIONS[$i]}"
            fi
        done
        echo ""

        IFS= read -rsn1 KEY </dev/tty || true
        if [[ $KEY == $'\x1b' ]]; then
            read -rsn2 -t 0.1 REST </dev/tty || true
            KEY+="$REST"
        fi

        case "$KEY" in
            $'\x1b[A'|[kK]) # UP
                if [[ $STAT_CHOICE -gt 0 ]]; then
                    STAT_CHOICE=$((STAT_CHOICE - 1))
                else
                    STAT_CHOICE=$((${#STAT_OPTIONS[@]} - 1))
                fi
                ;;
            $'\x1b[B'|[jJ]) # DOWN
                if [[ $STAT_CHOICE -lt $((${#STAT_OPTIONS[@]} - 1)) ]]; then
                    STAT_CHOICE=$((STAT_CHOICE + 1))
                else
                    STAT_CHOICE=0
                fi
                ;;
            ""|$'\n') # ENTER
                case $STAT_CHOICE in
                    0) fetch_script "status.sh" all ; break ;;
                    1) fetch_script "status.sh" peers ; break ;;
                    2) fetch_script "status.sh" latency ; break ;;
                    3) fetch_script "status.sh" ports ; break ;;
                    4) fetch_script "status.sh" firewall ; break ;;
                    5) return ;;
                esac
                ;;
            1) fetch_script "status.sh" all ; break ;;
            2) fetch_script "status.sh" peers ; break ;;
            3) fetch_script "status.sh" latency ; break ;;
            4) fetch_script "status.sh" ports ; break ;;
            5) fetch_script "status.sh" firewall ; break ;;
            6|0|[qQ]) return ;;
        esac
    done

    echo ""
    read -r -p "Press ENTER to return to menu..." </dev/tty || true
}

# Submenu: Doctor & Diagnostics
doctor_menu() {
    local DOC_CHOICE=0
    local DOC_OPTIONS=(
        "Run Complete 6-Point Doctor Scan (All Checks + Auto-Fix)"
        "Inspect Kernel Packet Forwarding & Sysctl Settings"
        "Inspect WireGuard Interface & Cryptographic Keys"
        "Inspect Docker Container Bridge & rinetd Port Maps"
        "Inspect Gateway Port Forwarding & NAT Table"
        "Back to Main Menu"
    )

    while true; do
        draw_header
        echo -e "${YELLOW}${BOLD}  🩺  WIREDNET DOCTOR & SYSTEM INSPECTOR SUBMENU${NC}"
        show_guide "Doctor Diagnostic Guide" \
                   "Performs deep health inspections on kernel, tunnel, Docker bindings, and routing." \
                   "If any issue is detected, WireNet Doctor offers an instant 1-click auto-repair."

        echo -e "  Use ${BOLD}UP/DOWN Arrow Keys${NC} and press ${BOLD}ENTER${NC} (or type a number):\n"

        for i in "${!DOC_OPTIONS[@]}"; do
            if [[ $i -eq $DOC_CHOICE ]]; then
                echo -e "  ${CYAN}${BOLD}▶ ${REV} [ $((i + 1)) ] ${DOC_OPTIONS[$i]} ${NC}"
            else
                echo -e "    [ $((i + 1)) ] ${DOC_OPTIONS[$i]}"
            fi
        done
        echo ""

        IFS= read -rsn1 KEY </dev/tty || true
        if [[ $KEY == $'\x1b' ]]; then
            read -rsn2 -t 0.1 REST </dev/tty || true
            KEY+="$REST"
        fi

        case "$KEY" in
            $'\x1b[A'|[kK]) # UP
                if [[ $DOC_CHOICE -gt 0 ]]; then
                    DOC_CHOICE=$((DOC_CHOICE - 1))
                else
                    DOC_CHOICE=$((${#DOC_OPTIONS[@]} - 1))
                fi
                ;;
            $'\x1b[B'|[jJ]) # DOWN
                if [[ $DOC_CHOICE -lt $((${#DOC_OPTIONS[@]} - 1)) ]]; then
                    DOC_CHOICE=$((DOC_CHOICE + 1))
                else
                    DOC_CHOICE=0
                fi
                ;;
            ""|$'\n') # ENTER
                case $DOC_CHOICE in
                    0) fetch_script "doctor.sh" ; break ;;
                    1)
                        echo -e "\n${BOLD}--- Kernel IP Forwarding & Localnet Settings ---${NC}"
                        sysctl net.ipv4.ip_forward net.ipv4.conf.all.route_localnet net.ipv4.conf.all.rp_filter
                        break
                        ;;
                    2)
                        echo -e "\n${BOLD}--- WireGuard Configuration & Status ---${NC}"
                        wg show wg0 2>/dev/null || echo "Interface wg0 is down."
                        ip addr show dev wg0 2>/dev/null || true
                        break
                        ;;
                    3)
                        echo -e "\n${BOLD}--- Docker & rinetd Port Bridge Inspection ---${NC}"
                        systemctl status rinetd --no-pager 2>/dev/null || true
                        cat /etc/rinetd.conf 2>/dev/null | head -n 15 || echo "No rinetd.conf found."
                        break
                        ;;
                    4)
                        echo -e "\n${BOLD}--- Gateway NAT Table & Port Forwarding Rules ---${NC}"
                        iptables -t nat -L PREROUTING -n -v --line-numbers 2>/dev/null | grep -E "10.200|dpt" || echo "No DNAT rules."
                        iptables -t nat -L POSTROUTING -n -v --line-numbers 2>/dev/null | grep "MASQUERADE" || true
                        break
                        ;;
                    5) return ;;
                esac
                ;;
            1) fetch_script "doctor.sh" ; break ;;
            2)
                sysctl net.ipv4.ip_forward net.ipv4.conf.all.route_localnet net.ipv4.conf.all.rp_filter
                break
                ;;
            3)
                wg show wg0 2>/dev/null || echo "Interface wg0 is down."
                ip addr show dev wg0 2>/dev/null || true
                break
                ;;
            4)
                systemctl status rinetd --no-pager 2>/dev/null || true
                cat /etc/rinetd.conf 2>/dev/null | head -n 15 || echo "No rinetd.conf found."
                break
                ;;
            5)
                iptables -t nat -L PREROUTING -n -v --line-numbers 2>/dev/null | grep -E "10.200|dpt" || echo "No DNAT rules."
                iptables -t nat -L POSTROUTING -n -v --line-numbers 2>/dev/null | grep "MASQUERADE" || true
                break
                ;;
            6|0|[qQ]) return ;;
        esac
    done

    echo ""
    read -r -p "Press ENTER to return to menu..." </dev/tty || true
}

# Submenu: Port & Multi-IP Routing
routing_menu() {
    local ROUTE_CHOICE=0
    local ROUTE_OPTIONS=(
        "Add Custom Port Forward (e.g. Gateway:25567 -> Node:25565)"
        "Map Dedicated Public IP to Specific Backend Node"
        "List All Active Port Forwards & DNAT Rules"
        "Flush & Reset All Port Forwarding Rules"
        "Back to Main Menu"
    )

    while true; do
        draw_header
        echo -e "${YELLOW}${BOLD}  🔀  CUSTOM PORT & MULTI-IP ROUTING SUBMENU${NC}"
        show_guide "Port Forwarding & Multi-IP Guide" \
                   "Route specific Gateway ports or dedicated Public IPs directly to backend nodes." \
                   "Allows multiple customer servers to use port 25565 on different Public IPs or ports."

        echo -e "  Use ${BOLD}UP/DOWN Arrow Keys${NC} and press ${BOLD}ENTER${NC} (or type a number):\n"

        for i in "${!ROUTE_OPTIONS[@]}"; do
            if [[ $i -eq $ROUTE_CHOICE ]]; then
                echo -e "  ${CYAN}${BOLD}▶ ${REV} [ $((i + 1)) ] ${ROUTE_OPTIONS[$i]} ${NC}"
            else
                echo -e "    [ $((i + 1)) ] ${ROUTE_OPTIONS[$i]}"
            fi
        done
        echo ""

        IFS= read -rsn1 KEY </dev/tty || true
        if [[ $KEY == $'\x1b' ]]; then
            read -rsn2 -t 0.1 REST </dev/tty || true
            KEY+="$REST"
        fi

        case "$KEY" in
            $'\x1b[A'|[kK]) # UP
                if [[ $ROUTE_CHOICE -gt 0 ]]; then
                    ROUTE_CHOICE=$((ROUTE_CHOICE - 1))
                else
                    ROUTE_CHOICE=$((${#ROUTE_OPTIONS[@]} - 1))
                fi
                ;;
            $'\x1b[B'|[jJ]) # DOWN
                if [[ $ROUTE_CHOICE -lt $((${#ROUTE_OPTIONS[@]} - 1)) ]]; then
                    ROUTE_CHOICE=$((ROUTE_CHOICE + 1))
                else
                    ROUTE_CHOICE=0
                fi
                ;;
            ""|$'\n') # ENTER
                case $ROUTE_CHOICE in
                    0) # Add Port Forward
                        echo -e "\n${BOLD}Enter Custom Port Translation Details:${NC}"
                        read -r -p "1. Enter Public Gateway Port (e.g. 25567): " GW_PORT </dev/tty
                        read -r -p "2. Enter Target Node WireGuard IP (e.g. 10.200.0.3): " DEST_NODE </dev/tty
                        read -r -p "3. Enter Local Container Port on Node (e.g. 25565): " LOCAL_PORT </dev/tty
                        if [[ -n "$GW_PORT" && -n "$DEST_NODE" && -n "$LOCAL_PORT" ]]; then
                            fetch_script "forward-port.sh" "$GW_PORT" "$DEST_NODE" "$LOCAL_PORT"
                        fi
                        break
                        ;;
                    1) # Multi-IP Mapping
                        echo -e "\n${BOLD}Enter Dedicated Public IP Mapping Details:${NC}"
                        read -r -p "1. Enter Dedicated Public IP on Gateway: " DEDICATED_IP </dev/tty
                        read -r -p "2. Enter Target Node Virtual IP (e.g. 10.200.0.3): " TARGET_NODE </dev/tty
                        if [[ -n "$DEDICATED_IP" && -n "$TARGET_NODE" ]]; then
                            fetch_script "add-ip-mapping.sh" "$DEDICATED_IP" "$TARGET_NODE"
                        fi
                        break
                        ;;
                    2) # List Rules
                        echo -e "\n${BOLD}--- Active DNAT & Port Forwarding Rules ---${NC}"
                        iptables -t nat -L PREROUTING -n -v --line-numbers 2>/dev/null | grep -E "dpt|10.200" || echo "No active port forward rules."
                        break
                        ;;
                    3) # Flush Rules
                        echo "[+] Flushing DNAT tables..."
                        iptables -t nat -F PREROUTING 2>/dev/null || true
                        echo -e "${GREEN}[✓] Port forwarding table reset.${NC}"
                        break
                        ;;
                    4) return ;;
                esac
                ;;
            1)
                read -r -p "1. Enter Public Gateway Port (e.g. 25567): " GW_PORT </dev/tty
                read -r -p "2. Enter Target Node WireGuard IP (e.g. 10.200.0.3): " DEST_NODE </dev/tty
                read -r -p "3. Enter Local Container Port on Node (e.g. 25565): " LOCAL_PORT </dev/tty
                if [[ -n "$GW_PORT" && -n "$DEST_NODE" && -n "$LOCAL_PORT" ]]; then
                    fetch_script "forward-port.sh" "$GW_PORT" "$DEST_NODE" "$LOCAL_PORT"
                fi
                break
                ;;
            2)
                read -r -p "1. Enter Dedicated Public IP on Gateway: " DEDICATED_IP </dev/tty
                read -r -p "2. Enter Target Node Virtual IP (e.g. 10.200.0.3): " TARGET_NODE </dev/tty
                if [[ -n "$DEDICATED_IP" && -n "$TARGET_NODE" ]]; then
                    fetch_script "add-ip-mapping.sh" "$DEDICATED_IP" "$TARGET_NODE"
                fi
                break
                ;;
            3)
                iptables -t nat -L PREROUTING -n -v --line-numbers 2>/dev/null | grep -E "dpt|10.200" || echo "No active port forward rules."
                break
                ;;
            4)
                iptables -t nat -F PREROUTING 2>/dev/null || true
                echo -e "${GREEN}[✓] Port forwarding table reset.${NC}"
                break
                ;;
            5|0|[qQ]) return ;;
        esac
    done

    echo ""
    read -r -p "Press ENTER to return to menu..." </dev/tty || true
}

# Submenu: Anti-DDoS Firewall
firewall_menu() {
    local FW_CHOICE=0
    local FW_OPTIONS=(
        "View Live Attack Telemetry & Dropped Packet Counters"
        "Enable Standard Protection (SYN Cookies + Rate Limiting)"
        "Enable STRICT Protection (Bot Raid & Flood Mitigation)"
        "Disable Shield (Pass-Through / Diagnostic Mode)"
        "Back to Main Menu"
    )

    while true; do
        draw_header
        echo -e "${YELLOW}${BOLD}  🛡️  MINECRAFT ANTI-DDOS FIREWALL SHIELD MANAGER (by UG88)${NC}"
        show_guide "Anti-DDoS Shield Guide" \
                   "Hardware-level SYN flood protection and malformed packet filters running in the Linux kernel." \
                   "Switch to 'STRICT' mode during heavy bot attacks with ZERO player disconnects."

        echo -e "  Use ${BOLD}UP/DOWN Arrow Keys${NC} and press ${BOLD}ENTER${NC} (or type a number):\n"

        for i in "${!FW_OPTIONS[@]}"; do
            if [[ $i -eq $FW_CHOICE ]]; then
                echo -e "  ${CYAN}${BOLD}▶ ${REV} [ $((i + 1)) ] ${FW_OPTIONS[$i]} ${NC}"
            else
                echo -e "    [ $((i + 1)) ] ${FW_OPTIONS[$i]}"
            fi
        done
        echo ""

        IFS= read -rsn1 KEY </dev/tty || true
        if [[ $KEY == $'\x1b' ]]; then
            read -rsn2 -t 0.1 REST </dev/tty || true
            KEY+="$REST"
        fi

        case "$KEY" in
            $'\x1b[A'|[kK]) # UP
                if [[ $FW_CHOICE -gt 0 ]]; then
                    FW_CHOICE=$((FW_CHOICE - 1))
                else
                    FW_CHOICE=$((${#FW_OPTIONS[@]} - 1))
                fi
                ;;
            $'\x1b[B'|[jJ]) # DOWN
                if [[ $FW_CHOICE -lt $((${#FW_OPTIONS[@]} - 1)) ]]; then
                    FW_CHOICE=$((FW_CHOICE + 1))
                else
                    FW_CHOICE=0
                fi
                ;;
            ""|$'\n') # ENTER
                case $FW_CHOICE in
                    0) fetch_script "firewall.sh" status ; break ;;
                    1) fetch_script "firewall.sh" enable ; break ;;
                    2) fetch_script "firewall.sh" strict ; break ;;
                    3) fetch_script "firewall.sh" disable ; break ;;
                    4) return ;;
                esac
                ;;
            1) fetch_script "firewall.sh" status ; break ;;
            2) fetch_script "firewall.sh" enable ; break ;;
            3) fetch_script "firewall.sh" strict ; break ;;
            4) fetch_script "firewall.sh" disable ; break ;;
            5|0|[qQ]) return ;;
        esac
    done

    echo ""
    read -r -p "Press ENTER to return to menu..." </dev/tty || true
}

# Submenu: Advanced Troubleshooting
troubleshoot_menu() {
    local SUB_CHOICE=0
    local SUB_OPTIONS=(
        "Universal Auto-Troubleshooter (Auto-Detects Role & Fixes Everything)"
        "Fix Gateway Port Forwarding & Routing Rules"
        "Fix Node Docker Port Bridge & rinetd Service"
        "Flush & Clean Conflicting IPTables NAT Chains"
        "Restart All WireNet & Tunnel Services"
        "Back to Main Menu"
    )

    while true; do
        draw_header
        echo -e "${YELLOW}${BOLD}  🛠️  ADVANCED TROUBLESHOOTING & REPAIR MENU${NC}"
        show_guide "Self-Healing Repair Guide" \
                   "Diagnoses and resolves broken routes, missing keys, Docker bridge NATs, and packet drops in 1 click." \
                   "Run 'Universal Auto-Troubleshooter' if you're not sure which server has an issue."

        echo -e "  Use ${BOLD}UP/DOWN Arrow Keys${NC} and press ${BOLD}ENTER${NC} (or type a number):\n"

        for i in "${!SUB_OPTIONS[@]}"; do
            if [[ $i -eq $SUB_CHOICE ]]; then
                echo -e "  ${CYAN}${BOLD}▶ ${REV} [ $((i + 1)) ] ${SUB_OPTIONS[$i]} ${NC}"
            else
                echo -e "    [ $((i + 1)) ] ${SUB_OPTIONS[$i]}"
            fi
        done
        echo ""

        IFS= read -rsn1 KEY </dev/tty || true
        if [[ $KEY == $'\x1b' ]]; then
            read -rsn2 -t 0.1 REST </dev/tty || true
            KEY+="$REST"
        fi

        case "$KEY" in
            $'\x1b[A'|[kK]) # UP
                if [[ $SUB_CHOICE -gt 0 ]]; then
                    SUB_CHOICE=$((SUB_CHOICE - 1))
                else
                    SUB_CHOICE=$((${#SUB_OPTIONS[@]} - 1))
                fi
                ;;
            $'\x1b[B'|[jJ]) # DOWN
                if [[ $SUB_CHOICE -lt $((${#SUB_OPTIONS[@]} - 1)) ]]; then
                    SUB_CHOICE=$((SUB_CHOICE + 1))
                else
                    SUB_CHOICE=0
                fi
                ;;
            ""|$'\n') # ENTER
                case $SUB_CHOICE in
                    0) fetch_script "troubleshoot.sh" ; break ;;
                    1) fetch_script "troubleshoot-gateway.sh" ; break ;;
                    2) fetch_script "troubleshoot-node.sh" ; break ;;
                    3)
                        echo "[+] Flushing stale NAT and Mangle tables..."
                        iptables -t nat -F PREROUTING 2>/dev/null || true
                        iptables -t nat -F POSTROUTING 2>/dev/null || true
                        iptables -t mangle -F 2>/dev/null || true
                        echo -e "${GREEN}[✓] IPTables tables cleared successfully.${NC}"
                        break
                        ;;
                    4)
                        echo "[+] Restarting all WireNet services..."
                        systemctl restart wg-quick@wg0 2>/dev/null || true
                        systemctl restart rinetd 2>/dev/null || true
                        echo -e "${GREEN}[✓] Services restarted successfully.${NC}"
                        break
                        ;;
                    5) return ;;
                esac
                ;;
            1) fetch_script "troubleshoot.sh" ; break ;;
            2) fetch_script "troubleshoot-gateway.sh" ; break ;;
            3) fetch_script "troubleshoot-node.sh" ; break ;;
            4)
                iptables -t nat -F PREROUTING 2>/dev/null || true
                iptables -t nat -F POSTROUTING 2>/dev/null || true
                iptables -t mangle -F 2>/dev/null || true
                echo -e "${GREEN}[✓] IPTables tables cleared successfully.${NC}"
                break
                ;;
            5)
                systemctl restart wg-quick@wg0 2>/dev/null || true
                systemctl restart rinetd 2>/dev/null || true
                echo -e "${GREEN}[✓] Services restarted successfully.${NC}"
                break
                ;;
            6|0|[qQ]) return ;;
        esac
    done

    echo ""
    read -r -p "Press ENTER to return to menu..." </dev/tty || true
}

# Main Menu Options List
MAIN_OPTIONS=(
    "Install / Setup Gateway VPS (Hub)"
    "Install / Setup Pterodactyl Node VPS (Spoke)"
    "Live Status & Telemetry Dashboard (Submenu)"
    "WireNet Doctor & System Inspector (Submenu)"
    "Minecraft Anti-DDoS Shield Manager (Submenu)"
    "Custom Port & Multi-IP Routing Manager (Submenu)"
    "Advanced Troubleshooting & Auto-Repair (Submenu)"
    "Uninstall WireNet Completely"
    "Exit WireNet Manager"
)

# Main Navigation Loop
CURRENT_INDEX=0

while true; do
    draw_header
    echo -e "  Use ${BOLD}UP/DOWN Arrow Keys${NC} to select, press ${BOLD}ENTER${NC} (or press number ${BOLD}1-9${NC}):\n"

    for i in "${!MAIN_OPTIONS[@]}"; do
        if [[ $i -eq $CURRENT_INDEX ]]; then
            echo -e "  ${CYAN}${BOLD}▶ ${REV} [ $((i + 1)) ] ${MAIN_OPTIONS[$i]} ${NC}"
        else
            echo -e "    [ $((i + 1)) ] ${MAIN_OPTIONS[$i]}"
        fi
    done
    echo ""
    echo -e "${WHITE}--------------------------------------------------------------------------------${NC}"

    IFS= read -rsn1 KEY </dev/tty || true
    if [[ $KEY == $'\x1b' ]]; then
        read -rsn2 -t 0.1 REST </dev/tty || true
        KEY+="$REST"
    fi

    case "$KEY" in
        $'\x1b[A'|[kK]) # UP Arrow
            if [[ $CURRENT_INDEX -gt 0 ]]; then
                CURRENT_INDEX=$((CURRENT_INDEX - 1))
            else
                CURRENT_INDEX=$((${#MAIN_OPTIONS[@]} - 1))
            fi
            ;;
        $'\x1b[B'|[jJ]) # DOWN Arrow
            if [[ $CURRENT_INDEX -lt $((${#MAIN_OPTIONS[@]} - 1)) ]]; then
                CURRENT_INDEX=$((CURRENT_INDEX + 1))
            else
                CURRENT_INDEX=0
            fi
            ;;
        ""|$'\n') # ENTER Key
            case $CURRENT_INDEX in
                0) # Install Gateway
                    draw_header
                    show_guide "Gateway VPS (Hub) Installation Guide" \
                               "Installs WireGuard Hub (10.200.0.1), Anti-DDoS SYN filter, and automated game port routing on your public AWS/Cloud VPS." \
                               "Run this once on your Public Gateway VPS."
                    read -r -p "Ready to begin Gateway setup? [Y/n]: " CONFIRM </dev/tty || CONFIRM="Y"
                    if [[ "$CONFIRM" =~ ^[Yy]?$ ]]; then
                        fetch_script "setup-gateway.sh"
                    fi
                    read -r -p "Press ENTER to continue..." </dev/tty || true
                    ;;
                1) # Install Node
                    draw_header
                    show_guide "Pterodactyl Node VPS (Spoke) Installation Guide" \
                               "Connects your Pterodactyl Node to the Gateway and automatically bridges Minecraft Docker ports (25565-25600, 30000-40050)." \
                               "Run this on each backend Node VPS where Pterodactyl Wings is running."
                    read -r -p "Ready to begin Node setup? [Y/n]: " CONFIRM </dev/tty || CONFIRM="Y"
                    if [[ "$CONFIRM" =~ ^[Yy]?$ ]]; then
                        fetch_script "setup-node.sh"
                    fi
                    read -r -p "Press ENTER to continue..." </dev/tty || true
                    ;;
                2) status_menu ;;
                3) doctor_menu ;;
                4) firewall_menu ;;
                5) routing_menu ;;
                6) troubleshoot_menu ;;
                7) # Uninstall
                    draw_header
                    show_guide "Complete Uninstallation Guide" \
                               "Completely stops and wipes WireNet services, WireGuard keys, and port forwarding rules." \
                               "Safe uninstaller will restore your server to its original network state."
                    read -r -p "Are you sure you want to completely uninstall WireNet? [y/N]: " CONFIRM </dev/tty || CONFIRM="N"
                    if [[ "$CONFIRM" =~ ^[Yy]$ ]]; then
                        fetch_script "uninstall.sh"
                    fi
                    read -r -p "Press ENTER to continue..." </dev/tty || true
                    ;;
                8) # Exit
                    echo -e "\n${GREEN}Thank you for using WireNet by UG88! Exiting.${NC}\n"
                    exit 0
                    ;;
            esac
            ;;
        1)
            draw_header
            show_guide "Gateway VPS (Hub) Installation Guide" \
                       "Installs WireGuard Hub (10.200.0.1), Anti-DDoS SYN filter, and automated game port routing on your public AWS/Cloud VPS." \
                       "Run this once on your Public Gateway VPS."
            read -r -p "Ready to begin Gateway setup? [Y/n]: " CONFIRM </dev/tty || CONFIRM="Y"
            if [[ "$CONFIRM" =~ ^[Yy]?$ ]]; then
                fetch_script "setup-gateway.sh"
            fi
            read -r -p "Press ENTER to continue..." </dev/tty || true
            ;;
        2)
            draw_header
            show_guide "Pterodactyl Node VPS (Spoke) Installation Guide" \
                       "Connects your Pterodactyl Node to the Gateway and automatically bridges Minecraft Docker ports (25565-25600, 30000-40050)." \
                       "Run this on each backend Node VPS where Pterodactyl Wings is running."
            read -r -p "Ready to begin Node setup? [Y/n]: " CONFIRM </dev/tty || CONFIRM="Y"
            if [[ "$CONFIRM" =~ ^[Yy]?$ ]]; then
                fetch_script "setup-node.sh"
            fi
            read -r -p "Press ENTER to continue..." </dev/tty || true
            ;;
        3) status_menu ;;
        4) doctor_menu ;;
        5) firewall_menu ;;
        6) routing_menu ;;
        7) troubleshoot_menu ;;
        8)
            draw_header
            show_guide "Complete Uninstallation Guide" \
                       "Completely stops and wipes WireNet services, WireGuard keys, and port forwarding rules." \
                       "Safe uninstaller will restore your server to its original network state."
            read -r -p "Are you sure you want to completely uninstall WireNet? [y/N]: " CONFIRM </dev/tty || CONFIRM="N"
            if [[ "$CONFIRM" =~ ^[Yy]$ ]]; then
                fetch_script "uninstall.sh"
            fi
            read -r -p "Press ENTER to continue..." </dev/tty || true
            ;;
        0|[qQ])
            echo -e "\n${GREEN}Thank you for using WireNet by UG88! Exiting.${NC}\n"
            exit 0
            ;;
    esac
done
