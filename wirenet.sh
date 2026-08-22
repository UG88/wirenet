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

REPO_BASE="https://raw.githubusercontent.com/UG88/wirenet/main"

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

# Main Menu Options
OPTIONS=(
    "Install / Setup Gateway VPS (Hub)"
    "Install / Setup Pterodactyl Node VPS (Spoke)"
    "Live Status & Telemetry Dashboard"
    "Run WireNet Doctor (Full Diagnostic & Auto-Repair)"
    "Minecraft Anti-DDoS Shield Manager"
    "Custom Port Forwarder (e.g. Gateway:25567 -> Node:25565)"
    "Dedicated Multi-IP Mapping (Public IP -> Node)"
    "Advanced Troubleshooting & Self-Healing Menu"
    "Uninstall WireNet Completely"
    "Exit WireNet Manager"
)

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

# Main Navigation Loop
CURRENT_INDEX=0

while true; do
    draw_header
    echo -e "  Use ${BOLD}UP/DOWN Arrow Keys${NC} to select, press ${BOLD}ENTER${NC} (or press number ${BOLD}1-9${NC}):\n"

    for i in "${!OPTIONS[@]}"; do
        if [[ $i -eq $CURRENT_INDEX ]]; then
            echo -e "  ${CYAN}${BOLD}▶ ${REV} [ $((i + 1)) ] ${OPTIONS[$i]} ${NC}"
        else
            echo -e "    [ $((i + 1)) ] ${OPTIONS[$i]}"
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
                CURRENT_INDEX=$((${#OPTIONS[@]} - 1))
            fi
            ;;
        $'\x1b[B'|[jJ]) # DOWN Arrow
            if [[ $CURRENT_INDEX -lt $((${#OPTIONS[@]} - 1)) ]]; then
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
                2) # Status Dashboard
                    fetch_script "status.sh"
                    read -r -p "Press ENTER to continue..." </dev/tty || true
                    ;;
                3) # Doctor Scan
                    fetch_script "doctor.sh"
                    read -r -p "Press ENTER to continue..." </dev/tty || true
                    ;;
                4) # Anti-DDoS Firewall
                    firewall_menu
                    ;;
                5) # Port Forwarding / Translation
                    draw_header
                    show_guide "Custom Port Translation Guide" \
                               "Forward any public Gateway port to any backend Node and local port." \
                               "Gateway:25567 -> Node 2 (10.200.0.3):25565"

                    echo -e "${BOLD}Enter Port Translation Details:${NC}"
                    read -r -p "1. Enter Public Gateway Port (e.g. 25567): " GW_PORT </dev/tty
                    read -r -p "2. Enter Target Node WireGuard IP (e.g. 10.200.0.3): " DEST_NODE </dev/tty
                    read -r -p "3. Enter Local Container Port on Node (e.g. 25565): " LOCAL_PORT </dev/tty

                    if [[ -n "$GW_PORT" && -n "$DEST_NODE" && -n "$LOCAL_PORT" ]]; then
                        fetch_script "forward-port.sh" "$GW_PORT" "$DEST_NODE" "$LOCAL_PORT"
                    else
                        echo -e "${RED}[-] Error: Port fields cannot be empty.${NC}"
                    fi
                    read -r -p "Press ENTER to continue..." </dev/tty || true
                    ;;
                6) # Multi-IP Mapping
                    draw_header
                    show_guide "Dedicated Multi-IP Mapping Guide" \
                               "Route secondary public IPs on your Gateway directly to specific backend nodes so each customer VM gets default port 25565." \
                               "Public IP 3.108.50.22 -> Node 2 (10.200.0.3)"

                    echo -e "${BOLD}Enter Dedicated IP Mapping Details:${NC}"
                    read -r -p "1. Enter Dedicated Public IP on Gateway: " DEDICATED_IP </dev/tty
                    read -r -p "2. Enter Target Node Virtual IP (e.g. 10.200.0.3): " TARGET_NODE </dev/tty

                    if [[ -n "$DEDICATED_IP" && -n "$TARGET_NODE" ]]; then
                        fetch_script "add-ip-mapping.sh" "$DEDICATED_IP" "$TARGET_NODE"
                    else
                        echo -e "${RED}[-] Error: IP fields cannot be empty.${NC}"
                    fi
                    read -r -p "Press ENTER to continue..." </dev/tty || true
                    ;;
                7) # Troubleshooting Submenu
                    troubleshoot_menu
                    ;;
                8) # Uninstall
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
                9) # Exit
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
        3) fetch_script "status.sh" ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
        4) fetch_script "doctor.sh" ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
        5) firewall_menu ;;
        6)
            draw_header
            show_guide "Custom Port Translation Guide" \
                       "Forward any public Gateway port to any backend Node and local port." \
                       "Gateway:25567 -> Node 2 (10.200.0.3):25565"

            echo -e "${BOLD}Enter Port Translation Details:${NC}"
            read -r -p "1. Enter Public Gateway Port (e.g. 25567): " GW_PORT </dev/tty
            read -r -p "2. Enter Target Node WireGuard IP (e.g. 10.200.0.3): " DEST_NODE </dev/tty
            read -r -p "3. Enter Local Container Port on Node (e.g. 25565): " LOCAL_PORT </dev/tty

            if [[ -n "$GW_PORT" && -n "$DEST_NODE" && -n "$LOCAL_PORT" ]]; then
                fetch_script "forward-port.sh" "$GW_PORT" "$DEST_NODE" "$LOCAL_PORT"
            else
                echo -e "${RED}[-] Error: Port fields cannot be empty.${NC}"
            fi
            read -r -p "Press ENTER to continue..." </dev/tty || true
            ;;
        7)
            draw_header
            show_guide "Dedicated Multi-IP Mapping Guide" \
                       "Route secondary public IPs on your Gateway directly to specific backend nodes so each customer VM gets default port 25565." \
                       "Public IP 3.108.50.22 -> Node 2 (10.200.0.3)"

            echo -e "${BOLD}Enter Dedicated IP Mapping Details:${NC}"
            read -r -p "1. Enter Dedicated Public IP on Gateway: " DEDICATED_IP </dev/tty
            read -r -p "2. Enter Target Node Virtual IP (e.g. 10.200.0.3): " TARGET_NODE </dev/tty

            if [[ -n "$DEDICATED_IP" && -n "$TARGET_NODE" ]]; then
                fetch_script "add-ip-mapping.sh" "$DEDICATED_IP" "$TARGET_NODE"
            else
                echo -e "${RED}[-] Error: IP fields cannot be empty.${NC}"
            fi
            read -r -p "Press ENTER to continue..." </dev/tty || true
            ;;
        8) troubleshoot_menu ;;
        9)
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
