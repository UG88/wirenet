#!/usr/bin/env bash
# ==============================================================================
# WireNet Interactive Master Manager (All-In-One Control Center)
# Full TUI Menu with Arrow-Key Navigation, Diagnostics, Setup & Repair
# ==============================================================================

set -euo pipefail

# ANSI Colors & Formatting
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
WHITE='\033[1;37m'
BOLD='\033[1m'
REV='\033[7m'
NC='\033[0m'

if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}[-] Error: WireNet Manager must be run as root (or with sudo).${NC}" >&2
   exit 1
fi

REPO_BASE="https://raw.githubusercontent.com/UG88/wirenet/main"

# Function to draw header
draw_header() {
    clear
    echo -e "${CYAN}${BOLD}"
    echo "================================================================================"
    echo "       __      ___          _   _      _     __  __                             "
    echo "       \ \    / (_)        | \ | |    | |   |  \/  |                            "
    echo "        \ \  / / _ _ __ ___|  \| | ___| |_  | \  / | __ _ _ __   __ _  __ _ ___ "
    echo "         \ \/ / | | '__/ _ \ . \` |/ _ \ __| | |\/| |/ _\` | '_ \ / _\` |/ _\` / __|"
    echo "          \  /  | | | |  __/ |\  |  __/ |_  | |  | | (_| | | | | (_| | (_| \__ \\"
    echo "           \/   |_|_|  \___|_| \_|\___|\__| |_|  |_|\__,_|_| |_|\__,_|\__, |___/"
    echo "                                                                        __/ |   "
    echo "                                                                       |___/    "
    echo "               Kernel-Level Ingress & Anti-DDoS Shield for Pterodactyl          "
    echo "================================================================================"
    echo -e "${NC}"
}

# Main Menu Options
OPTIONS=(
    "Install / Setup Gateway VPS (Hub)"
    "Install / Setup Pterodactyl Node VPS (Spoke)"
    "Live Status & Telemetry Dashboard"
    "Run WireNet Doctor (Full Diagnostic & Auto-Fix)"
    "Minecraft Anti-DDoS Shield Manager"
    "Custom Port Forwarder (e.g. 25567 -> 25565)"
    "Dedicated Multi-IP Mapping Tool"
    "Advanced Troubleshooting & Auto-Repair Menu"
    "Uninstall WireNet Completely"
    "Exit WireNet Manager"
)

# Submenu: Troubleshooting
troubleshoot_menu() {
    local SUB_CHOICE=0
    local SUB_OPTIONS=(
        "Universal Smart Auto-Fix (Auto-Detect Role)"
        "Fix Gateway Forwarding & Firewall Rules"
        "Fix Node Docker Bridge & rinetd Routing"
        "Flush & Clean Conflicting IPTables NAT Rules"
        "Restart All WireNet & Tunnel Services"
        "Back to Main Menu"
    )

    while true; do
        draw_header
        echo -e "${YELLOW}${BOLD}  🛠️  ADVANCED TROUBLESHOOTING & REPAIR MENU${NC}"
        echo -e "  Use ${BOLD}UP/DOWN Arrow Keys${NC} to navigate and press ${BOLD}ENTER${NC} (or type a number):\n"

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
                    0) curl -fsSL "$REPO_BASE/troubleshoot.sh" | sudo bash ; break ;;
                    1) curl -fsSL "$REPO_BASE/troubleshoot-gateway.sh" | sudo bash ; break ;;
                    2) curl -fsSL "$REPO_BASE/troubleshoot-node.sh" | sudo bash ; break ;;
                    3)
                        echo "[+] Flushing NAT and Mangle tables..."
                        iptables -t nat -F PREROUTING 2>/dev/null || true
                        iptables -t nat -F POSTROUTING 2>/dev/null || true
                        iptables -t mangle -F 2>/dev/null || true
                        echo "[✓] IPTables tables cleared."
                        break
                        ;;
                    4)
                        echo "[+] Restarting services..."
                        systemctl restart wg-quick@wg0 2>/dev/null || true
                        systemctl restart rinetd 2>/dev/null || true
                        echo "[✓] Services restarted."
                        break
                        ;;
                    5) return ;;
                esac
                ;;
            1) curl -fsSL "$REPO_BASE/troubleshoot.sh" | sudo bash ; break ;;
            2) curl -fsSL "$REPO_BASE/troubleshoot-gateway.sh" | sudo bash ; break ;;
            3) curl -fsSL "$REPO_BASE/troubleshoot-node.sh" | sudo bash ; break ;;
            4)
                iptables -t nat -F PREROUTING 2>/dev/null || true
                iptables -t nat -F POSTROUTING 2>/dev/null || true
                iptables -t mangle -F 2>/dev/null || true
                echo "[✓] IPTables tables cleared."
                break
                ;;
            5)
                systemctl restart wg-quick@wg0 2>/dev/null || true
                systemctl restart rinetd 2>/dev/null || true
                echo "[✓] Services restarted."
                break
                ;;
            6|0|[qQ]) return ;;
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
        "Enable STRICT Protection (Bot Raid Mitigation)"
        "Disable Shield (Pass-Through Mode)"
        "Back to Main Menu"
    )

    while true; do
        draw_header
        echo -e "${YELLOW}${BOLD}  🛡️  MINECRAFT ANTI-DDOS FIREWALL SHIELD${NC}"
        echo -e "  Use ${BOLD}UP/DOWN Arrow Keys${NC} and press ${BOLD}ENTER${NC}:\n"

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
                    0) curl -fsSL "$REPO_BASE/firewall.sh" | sudo bash -s -- status ; break ;;
                    1) curl -fsSL "$REPO_BASE/firewall.sh" | sudo bash -s -- enable ; break ;;
                    2) curl -fsSL "$REPO_BASE/firewall.sh" | sudo bash -s -- strict ; break ;;
                    3) curl -fsSL "$REPO_BASE/firewall.sh" | sudo bash -s -- disable ; break ;;
                    4) return ;;
                esac
                ;;
            1) curl -fsSL "$REPO_BASE/firewall.sh" | sudo bash -s -- status ; break ;;
            2) curl -fsSL "$REPO_BASE/firewall.sh" | sudo bash -s -- enable ; break ;;
            3) curl -fsSL "$REPO_BASE/firewall.sh" | sudo bash -s -- strict ; break ;;
            4) curl -fsSL "$REPO_BASE/firewall.sh" | sudo bash -s -- disable ; break ;;
            5|0|[qQ]) return ;;
        esac
    done

    echo ""
    read -r -p "Press ENTER to return to menu..." </dev/tty || true
}

# Main Interactive Loop
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
                    curl -fsSL "$REPO_BASE/setup-gateway.sh" | sudo bash
                    read -r -p "Press ENTER to continue..." </dev/tty || true
                    ;;
                1) # Install Node
                    curl -fsSL "$REPO_BASE/setup-node.sh" | sudo bash
                    read -r -p "Press ENTER to continue..." </dev/tty || true
                    ;;
                2) # Status
                    curl -fsSL "$REPO_BASE/status.sh" | sudo bash
                    read -r -p "Press ENTER to continue..." </dev/tty || true
                    ;;
                3) # Doctor
                    curl -fsSL "$REPO_BASE/doctor.sh" | sudo bash
                    read -r -p "Press ENTER to continue..." </dev/tty || true
                    ;;
                4) # Anti-DDoS Firewall
                    firewall_menu
                    ;;
                5) # Port Forward
                    echo ""
                    read -r -p "Enter Gateway Public Port (e.g. 25567): " GW_PORT </dev/tty
                    read -r -p "Enter Node Virtual IP (e.g. 10.200.0.3): " DEST_NODE </dev/tty
                    read -r -p "Enter Local Server Port on Node (e.g. 25565): " LOCAL_PORT </dev/tty
                    curl -fsSL "$REPO_BASE/forward-port.sh" | sudo bash -s -- "$GW_PORT" "$DEST_NODE" "$LOCAL_PORT"
                    read -r -p "Press ENTER to continue..." </dev/tty || true
                    ;;
                6) # Multi-IP Mapping
                    echo ""
                    read -r -p "Enter Dedicated Public IP on Gateway: " DEDICATED_IP </dev/tty
                    read -r -p "Enter Target Node Virtual IP (e.g. 10.200.0.2): " TARGET_NODE </dev/tty
                    curl -fsSL "$REPO_BASE/add-ip-mapping.sh" | sudo bash -s -- "$DEDICATED_IP" "$TARGET_NODE"
                    read -r -p "Press ENTER to continue..." </dev/tty || true
                    ;;
                7) # Troubleshooting Submenu
                    troubleshoot_menu
                    ;;
                8) # Uninstall
                    curl -fsSL "$REPO_BASE/uninstall.sh" | sudo bash
                    read -r -p "Press ENTER to continue..." </dev/tty || true
                    ;;
                9) # Exit
                    echo -e "\n${GREEN}Thank you for using WireNet! Exiting.${NC}\n"
                    exit 0
                    ;;
            esac
            ;;
        1) curl -fsSL "$REPO_BASE/setup-gateway.sh" | sudo bash ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
        2) curl -fsSL "$REPO_BASE/setup-node.sh" | sudo bash ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
        3) curl -fsSL "$REPO_BASE/status.sh" | sudo bash ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
        4) curl -fsSL "$REPO_BASE/doctor.sh" | sudo bash ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
        5) firewall_menu ;;
        6)
            read -r -p "Enter Gateway Public Port (e.g. 25567): " GW_PORT </dev/tty
            read -r -p "Enter Node Virtual IP (e.g. 10.200.0.3): " DEST_NODE </dev/tty
            read -r -p "Enter Local Server Port on Node (e.g. 25565): " LOCAL_PORT </dev/tty
            curl -fsSL "$REPO_BASE/forward-port.sh" | sudo bash -s -- "$GW_PORT" "$DEST_NODE" "$LOCAL_PORT"
            read -r -p "Press ENTER to continue..." </dev/tty || true
            ;;
        7)
            read -r -p "Enter Dedicated Public IP on Gateway: " DEDICATED_IP </dev/tty
            read -r -p "Enter Target Node Virtual IP (e.g. 10.200.0.2): " TARGET_NODE </dev/tty
            curl -fsSL "$REPO_BASE/add-ip-mapping.sh" | sudo bash -s -- "$DEDICATED_IP" "$TARGET_NODE"
            read -r -p "Press ENTER to continue..." </dev/tty || true
            ;;
        8) troubleshoot_menu ;;
        9) curl -fsSL "$REPO_BASE/uninstall.sh" | sudo bash ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
        0|[qQ])
            echo -e "\n${GREEN}Thank you for using WireNet! Exiting.${NC}\n"
            exit 0
            ;;
    esac
done
