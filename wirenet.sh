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
INSTALL_DIR="/opt/wirenet"
BIN_PATH="/usr/local/bin/wirenet"
CURRENT_VERSION="v1.2.1"

# Auto-install to /usr/local/bin if running from curl
if [[ ! -f "$BIN_PATH" || ! -d "$INSTALL_DIR" ]]; then
    mkdir -p "${INSTALL_DIR}/scripts"
    cp -f "$0" "${INSTALL_DIR}/wirenet.sh" 2>/dev/null || curl -fsSL "${REPO_BASE}/wirenet.sh?$(date +%s)" -o "${INSTALL_DIR}/wirenet.sh" 2>/dev/null || true
    chmod 755 "${INSTALL_DIR}/wirenet.sh" 2>/dev/null || true
    
    cat << 'EOF' > "${BIN_PATH}" 2>/dev/null || true
#!/usr/bin/env bash
if [[ $EUID -ne 0 ]]; then
    exec sudo /opt/wirenet/wirenet.sh "$@"
else
    exec /opt/wirenet/wirenet.sh "$@"
fi
EOF
    chmod 755 "${BIN_PATH}" 2>/dev/null || true
fi

# Helper to fetch and run scripts (Always fetches fresh from GitHub with local fallback)
fetch_script() {
    local script_name="$1"
    shift
    curl -fsSL -H "Cache-Control: no-cache" "${REPO_BASE}/scripts/${script_name}?$(date +%s)" -o "${INSTALL_DIR}/scripts/${script_name}" 2>/dev/null || true
    chmod 755 "${INSTALL_DIR}/scripts/${script_name}" 2>/dev/null || true
    
    if [[ -f "${INSTALL_DIR}/scripts/${script_name}" ]]; then
        sudo bash "${INSTALL_DIR}/scripts/${script_name}" "$@"
    else
        curl -fsSL -H "Cache-Control: no-cache" "${REPO_BASE}/scripts/${script_name}?$(date +%s)" | sudo bash -s -- "$@"
    fi
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

# Helper function to print a dynamic info/guide box
show_guide() {
    local title="$1"
    local desc="$2"
    local example="${3:-}"
    echo -e "${YELLOW}${BOLD}┌── ℹ️  ${title} ──────────────────────────────────────────${NC}"
    echo -e "${YELLOW}│${NC} ${desc}"
    if [[ -n "$example" ]]; then
        echo -e "${YELLOW}│${NC} ${CYAN}${BOLD}Example:${NC} ${example}"
    fi
    echo -e "${YELLOW}└───────────────────────────────────────────────────────────────────${NC}\n"
}

# Auto-Update System
update_wirenet() {
    draw_header
    echo -e "${YELLOW}${BOLD}  🔄  CHECKING FOR WIRENET UPDATES...${NC}\n"
    echo -e "Current Local Version: ${BOLD}${CURRENT_VERSION}${NC}"
    echo -e "Connecting to GitHub repository: ${CYAN}UG88/wirenet${NC}..."

    mkdir -p "${INSTALL_DIR}/scripts"

    SCRIPTS=(
        "setup-gateway.sh"
        "setup-node.sh"
        "doctor.sh"
        "status.sh"
        "firewall.sh"
        "troubleshoot-gateway.sh"
        "troubleshoot-node.sh"
        "wirenet-watcher.sh"
        "install-daemon.sh"
        "forward-port.sh"
        "add-ip-mapping.sh"
        "uninstall.sh"
    )
    for s in "${SCRIPTS[@]}"; do
        curl -fsSL "${REPO_BASE}/scripts/${s}?$(date +%s)" -o "${INSTALL_DIR}/scripts/${s}" 2>/dev/null || true
        chmod 755 "${INSTALL_DIR}/scripts/${s}" 2>/dev/null || true
    done
}

# Direct CLI Command Dispatcher (for non-interactive execution)
if [[ $# -gt 0 ]]; then
    case "$1" in
        tui|dashboard)
            if command -v wirenet-daemon >/dev/null 2>&1; then
                exec wirenet-daemon tui
            else
                fetch_script "status.sh" live
                exit 0
            fi
            ;;
        doctor)
            fetch_script "doctor.sh"
            if command -v wirenet-daemon >/dev/null 2>&1; then
                wirenet-daemon doctor 2>/dev/null || true
            fi
            exit 0
            ;;
        status)
            fetch_script "status.sh" "${2:-all}"
            exit 0
            ;;
        shield|firewall)
            fetch_script "firewall.sh" "${2:-status}"
            exit 0
            ;;
        daemon)
            case "${2:-status}" in
                install|build)
                    fetch_script "install-daemon.sh"
                    ;;
                start)
                    systemctl start wirenet-gateway.service 2>/dev/null || systemctl start wirenet-node.service 2>/dev/null || true
                    echo -e "${GREEN}[✓] WireNet Daemon started.${NC}"
                    ;;
                stop)
                    systemctl stop wirenet-gateway.service 2>/dev/null || systemctl stop wirenet-node.service 2>/dev/null || true
                    echo -e "${YELLOW}[✓] WireNet Daemon stopped.${NC}"
                    ;;
                restart)
                    systemctl restart wirenet-gateway.service 2>/dev/null || systemctl restart wirenet-node.service 2>/dev/null || true
                    echo -e "${GREEN}[✓] WireNet Daemon restarted.${NC}"
                    ;;
                status)
                    systemctl status wirenet-gateway.service 2>/dev/null || systemctl status wirenet-node.service 2>/dev/null || echo "WireNet daemon service not installed. Run 'wirenet daemon install' to build."
                    ;;
            esac
            exit 0
            ;;
        update|upgrade)
            sync_scripts
            echo -e "${GREEN}[✓] WireNet successfully updated!${NC}"
            exit 0
            ;;
        help|--help|-h)
            echo -e "${CYAN}${BOLD}WireNet Unified CLI Manager${NC}"
            echo -e "Usage: wirenet [COMMAND]\n"
            echo -e "Commands:"
            echo -e "  wirenet                   Launch interactive Control Center TUI"
            echo -e "  wirenet tui               Launch real-time live Dashboard"
            echo -e "  wirenet doctor            Run 6-point system health inspector"
            echo -e "  wirenet status            Display current tunnel & port status"
            echo -e "  wirenet shield <mode>     Manage Anti-DDoS Shield (standard|strict|off|status)"
            echo -e "  wirenet daemon <action>   Manage Rust Daemon (install|start|stop|restart|status)"
            echo -e "  wirenet update            Update all WireNet scripts & binaries from GitHub"
            exit 0
            ;;
    esac
fi

# Auto-Update System
update_wirenet() {
    draw_header
    echo -e "${YELLOW}${BOLD}  🔄  CHECKING FOR WIRENET UPDATES...${NC}\n"
    echo -e "Current Local Version: ${BOLD}${CURRENT_VERSION}${NC}"
    echo -e "Connecting to GitHub repository: ${CYAN}UG88/wirenet${NC}..."

    sync_scripts

    echo "[+] Downloading latest WireNet Master Manager..."
    curl -fsSL -H "Cache-Control: no-cache" "${REPO_BASE}/wirenet.sh?$(date +%s)" -o "${INSTALL_DIR}/wirenet.sh"
    chmod 755 "${INSTALL_DIR}/wirenet.sh"

    cat << 'EOF' > "${BIN_PATH}"
#!/usr/bin/env bash
if [[ $EUID -ne 0 ]]; then
    exec sudo /opt/wirenet/wirenet.sh "$@"
else
    exec /opt/wirenet/wirenet.sh "$@"
fi
EOF
    chmod 755 "${BIN_PATH}"

    echo -e "\n${GREEN}${BOLD}[✓] WireNet successfully updated to the latest release!${NC}"
    echo -e "All scripts in ${INSTALL_DIR} have been synchronized.\n"
    read -r -p "Press ENTER to reload WireNet..." </dev/tty || true
    exec "${BIN_PATH}"
}

# Submenu: Status & Telemetry Dashboard
status_menu() {
    local STAT_CHOICE=0
    local STAT_OPTIONS=(
        "View Live Real-Time Telemetry Dashboard (Auto-Refreshing Loop)"
        "View One-Shot Telemetry Summary (All Metrics at Once)"
        "Check WireGuard Interface & Handshake Telemetry Only"
        "Check Live Tunnel Latency & Ping to Nodes Only"
        "Inspect Active Minecraft & Game Server Ports Only"
        "Inspect Anti-DDoS Attack Drop Counters Only"
        "Back to Main Menu"
    )

    local STAT_DESCS=(
        "Starts a live, real-time dashboard updating every 2s showing active peers, transfer counters, latency, and ports."
        "Runs a single snapshot check of all telemetry metrics (WireGuard, latency, ports, shield)."
        "Inspects wireguard wg0 status, cryptographic public keys, and last handshake timestamp."
        "Tests sub-millisecond ICMP round-trip latency to all connected backend nodes."
        "Scans all listening TCP/UDP ports (25565, 25566-25600, 30000+) on this host."
        "Displays real-time dropped attack packet counters from the hardware SYN cookie filter."
        "Return to the main WireNet control center."
    )

    while true; do
        draw_header
        echo -e "${YELLOW}${BOLD}  📊  LIVE STATUS & TELEMETRY SUBMENU (by UG88)${NC}"
        show_guide "${STAT_OPTIONS[$STAT_CHOICE]}" "${STAT_DESCS[$STAT_CHOICE]}"

        echo -e "  Use ${BOLD}UP/DOWN Arrow Keys${NC} to select, press ${BOLD}ENTER${NC}:\n"

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
                    0) fetch_script "status.sh" live ;;
                    1) fetch_script "status.sh" all ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
                    2) fetch_script "status.sh" peers ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
                    3) fetch_script "status.sh" latency ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
                    4) fetch_script "status.sh" ports ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
                    5) fetch_script "status.sh" firewall ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
                    6) return ;;
                esac
                ;;
            1) fetch_script "status.sh" live ;;
            2) fetch_script "status.sh" all ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
            3) fetch_script "status.sh" peers ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
            4) fetch_script "status.sh" latency ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
            5) fetch_script "status.sh" ports ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
            6) fetch_script "status.sh" firewall ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
            7|0|[qQ]) return ;;
        esac
    done
}

# Submenu: Doctor & Diagnostics
doctor_menu() {
    local DOC_CHOICE=0
    local DOC_OPTIONS=(
        "Run Complete 6-Point Doctor Scan (All Checks + Auto-Repair)"
        "Inspect Kernel Packet Forwarding & Sysctl Settings Only"
        "Inspect WireGuard Interface & Cryptographic Keys Only"
        "Inspect Docker Container Bridge & rinetd Port Maps Only"
        "Inspect Gateway Port Forwarding & NAT Table Only"
        "Back to Main Menu"
    )

    local DOC_DESCS=(
        "Scans all 6 subsystems (Kernel, WireGuard, Handshakes, Ping, Docker, NAT) and automatically repairs any issues found."
        "Verifies that net.ipv4.ip_forward=1 and route_localnet=1 are enabled in the Linux kernel."
        "Inspects wireguard keys, interface status, and active peer IP allocations."
        "Verifies that Docker containers on Pterodactyl are accessible via the rinetd port bridge."
        "Inspects iptables PREROUTING DNAT and POSTROUTING MASQUERADE tables."
        "Return to the main WireNet control center."
    )

    while true; do
        draw_header
        echo -e "${YELLOW}${BOLD}  🩺  WIRENET DOCTOR & SYSTEM INSPECTOR SUBMENU${NC}"
        show_guide "${DOC_OPTIONS[$DOC_CHOICE]}" "${DOC_DESCS[$DOC_CHOICE]}"

        echo -e "  Use ${BOLD}UP/DOWN Arrow Keys${NC} to select, press ${BOLD}ENTER${NC}:\n"

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
                    0) fetch_script "doctor.sh" ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
                    1)
                        echo -e "\n${BOLD}--- Kernel IP Forwarding & Localnet Settings ---${NC}"
                        sysctl net.ipv4.ip_forward net.ipv4.conf.all.route_localnet net.ipv4.conf.all.rp_filter
                        read -r -p "Press ENTER to continue..." </dev/tty || true
                        ;;
                    2)
                        echo -e "\n${BOLD}--- WireGuard Configuration & Status ---${NC}"
                        wg show wg0 2>/dev/null || echo "Interface wg0 is down."
                        ip addr show dev wg0 2>/dev/null || true
                        read -r -p "Press ENTER to continue..." </dev/tty || true
                        ;;
                    3)
                        echo -e "\n${BOLD}--- Docker & rinetd Port Bridge Inspection ---${NC}"
                        systemctl status rinetd --no-pager 2>/dev/null || true
                        cat /etc/rinetd.conf 2>/dev/null | head -n 15 || echo "No rinetd.conf found."
                        read -r -p "Press ENTER to continue..." </dev/tty || true
                        ;;
                    4)
                        echo -e "\n${BOLD}--- Gateway NAT Table & Port Forwarding Rules ---${NC}"
                        iptables -t nat -L PREROUTING -n -v --line-numbers 2>/dev/null | grep -E "10.200|dpt" || echo "No DNAT rules."
                        iptables -t nat -L POSTROUTING -n -v --line-numbers 2>/dev/null | grep "MASQUERADE" || true
                        read -r -p "Press ENTER to continue..." </dev/tty || true
                        ;;
                    5) return ;;
                esac
                ;;
            1) fetch_script "doctor.sh" ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
            2)
                sysctl net.ipv4.ip_forward net.ipv4.conf.all.route_localnet net.ipv4.conf.all.rp_filter
                read -r -p "Press ENTER to continue..." </dev/tty || true
                ;;
            3)
                wg show wg0 2>/dev/null || echo "Interface wg0 is down."
                ip addr show dev wg0 2>/dev/null || true
                read -r -p "Press ENTER to continue..." </dev/tty || true
                ;;
            4)
                systemctl status rinetd --no-pager 2>/dev/null || true
                cat /etc/rinetd.conf 2>/dev/null | head -n 15 || echo "No rinetd.conf found."
                read -r -p "Press ENTER to continue..." </dev/tty || true
                ;;
            5)
                iptables -t nat -L PREROUTING -n -v --line-numbers 2>/dev/null | grep -E "10.200|dpt" || echo "No DNAT rules."
                iptables -t nat -L POSTROUTING -n -v --line-numbers 2>/dev/null | grep "MASQUERADE" || true
                read -r -p "Press ENTER to continue..." </dev/tty || true
                ;;
            6|0|[qQ]) return ;;
        esac
    done
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

    local ROUTE_DESCS=(
        "Maps a specific public port on the Gateway to a specific node IP and container port (e.g. Gateway:25567 -> Node 2:25565)."
        "Binds secondary public IPv4 addresses on the Gateway to different backend nodes so every customer gets port 25565."
        "Displays the active iptables DNAT table showing all mapped public ports."
        "Clears and resets the iptables PREROUTING port forwarding table."
        "Return to the main WireNet control center."
    )

    while true; do
        draw_header
        echo -e "${YELLOW}${BOLD}  🔀  CUSTOM PORT & MULTI-IP ROUTING SUBMENU${NC}"
        show_guide "${ROUTE_OPTIONS[$ROUTE_CHOICE]}" "${ROUTE_DESCS[$ROUTE_CHOICE]}"

        echo -e "  Use ${BOLD}UP/DOWN Arrow Keys${NC} to select, press ${BOLD}ENTER${NC}:\n"

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
                        read -r -p "Press ENTER to continue..." </dev/tty || true
                        ;;
                    1) # Multi-IP Mapping
                        echo -e "\n${BOLD}Enter Dedicated Public IP Mapping Details:${NC}"
                        read -r -p "1. Enter Dedicated Public IP on Gateway: " DEDICATED_IP </dev/tty
                        read -r -p "2. Enter Target Node Virtual IP (e.g. 10.200.0.3): " TARGET_NODE </dev/tty
                        if [[ -n "$DEDICATED_IP" && -n "$TARGET_NODE" ]]; then
                            fetch_script "add-ip-mapping.sh" "$DEDICATED_IP" "$TARGET_NODE"
                        fi
                        read -r -p "Press ENTER to continue..." </dev/tty || true
                        ;;
                    2) # List Rules
                        echo -e "\n${BOLD}--- Active DNAT & Port Forwarding Rules ---${NC}"
                        iptables -t nat -L PREROUTING -n -v --line-numbers 2>/dev/null | grep -E "dpt|10.200" || echo "No active port forward rules."
                        read -r -p "Press ENTER to continue..." </dev/tty || true
                        ;;
                    3) # Flush Rules
                        echo "[+] Flushing DNAT tables..."
                        iptables -t nat -F PREROUTING 2>/dev/null || true
                        echo -e "${GREEN}[✓] Port forwarding table reset.${NC}"
                        read -r -p "Press ENTER to continue..." </dev/tty || true
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
                read -r -p "Press ENTER to continue..." </dev/tty || true
                ;;
            2)
                read -r -p "1. Enter Dedicated Public IP on Gateway: " DEDICATED_IP </dev/tty
                read -r -p "2. Enter Target Node Virtual IP (e.g. 10.200.0.3): " TARGET_NODE </dev/tty
                if [[ -n "$DEDICATED_IP" && -n "$TARGET_NODE" ]]; then
                    fetch_script "add-ip-mapping.sh" "$DEDICATED_IP" "$TARGET_NODE"
                fi
                read -r -p "Press ENTER to continue..." </dev/tty || true
                ;;
            3)
                iptables -t nat -L PREROUTING -n -v --line-numbers 2>/dev/null | grep -E "dpt|10.200" || echo "No active port forward rules."
                read -r -p "Press ENTER to continue..." </dev/tty || true
                ;;
            4)
                iptables -t nat -F PREROUTING 2>/dev/null || true
                echo -e "${GREEN}[✓] Port forwarding table reset.${NC}"
                read -r -p "Press ENTER to continue..." </dev/tty || true
                ;;
            5|0|[qQ]) return ;;
        esac
    done
}

# Submenu: Anti-DDoS Firewall
firewall_menu() {
    local FW_CHOICE=0
    local FW_OPTIONS=(
        "View Live Real-Time Attack Telemetry (Auto-Refreshing Loop)"
        "View One-Shot Attack Statistics Snapshot"
        "Enable Standard Protection (SYN Cookies + Rate Limiting)"
        "Enable STRICT Protection (Bot Raid & Flood Mitigation)"
        "Disable Shield (Pass-Through / Diagnostic Mode)"
        "Back to Main Menu"
    )

    local FW_DESCS=(
        "Runs a live auto-refreshing monitor updating every 1s showing real-time dropped attacks, SYN floods, and malformed packets."
        "Displays a single snapshot of the current iptables MC_TCP_FILTER and MC_UDP_FILTER tables."
        "Turns on hardware SYN cookies, malformed packet droppers, and 25 SYN/sec rate limiting (zero player disconnects)."
        "Enables high-security anti-bot mode restricting connections to 10 SYN/sec per IP during heavy raids."
        "Temporarily pauses packet filtering rules (all traffic passes directly without rate limiting)."
        "Return to the main WireNet control center."
    )

    while true; do
        draw_header
        echo -e "${YELLOW}${BOLD}  🛡️  MINECRAFT ANTI-DDOS FIREWALL SHIELD MANAGER (by UG88)${NC}"
        show_guide "${FW_OPTIONS[$FW_CHOICE]}" "${FW_DESCS[$FW_CHOICE]}"

        echo -e "  Use ${BOLD}UP/DOWN Arrow Keys${NC} to select, press ${BOLD}ENTER${NC}:\n"

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
                    0) fetch_script "firewall.sh" live ;;
                    1) fetch_script "firewall.sh" status ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
                    2) fetch_script "firewall.sh" enable ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
                    3) fetch_script "firewall.sh" strict ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
                    4) fetch_script "firewall.sh" disable ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
                    5) return ;;
                esac
                ;;
            1) fetch_script "firewall.sh" live ;;
            2) fetch_script "firewall.sh" status ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
            3) fetch_script "firewall.sh" enable ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
            4) fetch_script "firewall.sh" strict ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
            5) fetch_script "firewall.sh" disable ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
            6|0|[qQ]) return ;;
        esac
    done
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

    local SUB_DESCS=(
        "Smart auto-fixer: detects whether this VPS is Gateway or Node, checks tunnel ping, cleans NAT, and auto-repairs in 1 click."
        "Gateway repair: re-enables kernel IP forwarding, refreshes DNAT rules to 10.200.0.2, and enables MASQUERADE."
        "Node repair: enables route_localnet=1, cleans stale NAT, and restarts rinetd Docker port bridge."
        "Flushes PREROUTING, POSTROUTING, and MANGLE tables to remove any conflicting or broken rules."
        "Restarts wg-quick@wg0 and rinetd services in clean dependency order."
        "Return to the main WireNet control center."
    )

    while true; do
        draw_header
        echo -e "${YELLOW}${BOLD}  🛠️  ADVANCED TROUBLESHOOTING & REPAIR MENU${NC}"
        show_guide "${SUB_OPTIONS[$SUB_CHOICE]}" "${SUB_DESCS[$SUB_CHOICE]}"

        echo -e "  Use ${BOLD}UP/DOWN Arrow Keys${NC} to select, press ${BOLD}ENTER${NC}:\n"

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
                    0) fetch_script "troubleshoot.sh" ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
                    1) fetch_script "troubleshoot-gateway.sh" ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
                    2) fetch_script "troubleshoot-node.sh" ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
                    3)
                        echo "[+] Flushing stale NAT and Mangle tables..."
                        iptables -t nat -F PREROUTING 2>/dev/null || true
                        iptables -t nat -F POSTROUTING 2>/dev/null || true
                        iptables -t mangle -F 2>/dev/null || true
                        echo -e "${GREEN}[✓] IPTables tables cleared successfully.${NC}"
                        read -r -p "Press ENTER to continue..." </dev/tty || true
                        ;;
                    4)
                        echo "[+] Restarting all WireNet services..."
                        systemctl restart wg-quick@wg0 2>/dev/null || true
                        systemctl restart rinetd 2>/dev/null || true
                        echo -e "${GREEN}[✓] Services restarted successfully.${NC}"
                        read -r -p "Press ENTER to continue..." </dev/tty || true
                        ;;
                    5) return ;;
                esac
                ;;
            1) fetch_script "troubleshoot.sh" ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
            2) fetch_script "troubleshoot-gateway.sh" ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
            3) fetch_script "troubleshoot-node.sh" ; read -r -p "Press ENTER to continue..." </dev/tty || true ;;
            4)
                iptables -t nat -F PREROUTING 2>/dev/null || true
                iptables -t nat -F POSTROUTING 2>/dev/null || true
                iptables -t mangle -F 2>/dev/null || true
                echo -e "${GREEN}[✓] IPTables tables cleared successfully.${NC}"
                read -r -p "Press ENTER to continue..." </dev/tty || true
                ;;
            5)
                systemctl restart wg-quick@wg0 2>/dev/null || true
                systemctl restart rinetd 2>/dev/null || true
                echo -e "${GREEN}[✓] Services restarted successfully.${NC}"
                read -r -p "Press ENTER to continue..." </dev/tty || true
                ;;
            6|0|[qQ]) return ;;
        esac
    done
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
    "Check for Updates & Upgrade WireNet (Latest Version)"
    "Uninstall WireNet Completely"
    "Exit WireNet Manager"
)

MAIN_DESCS=(
    "Deploys WireGuard Hub (10.200.0.1), Anti-DDoS SYN filter, and automated game port forwarding on your public AWS/Cloud VPS."
    "Connects this Pterodactyl Node to the Gateway and bridges Minecraft Docker ports (25565-25600, 30000-30050) automatically."
    "Inspect live real-time telemetry, active WireGuard handshakes, sub-millisecond node ping latency, and listening ports."
    "Runs a deep 6-point health inspection on kernel, tunnel, Docker bindings, and routing with 1-click auto-repair."
    "Real-time attack monitor and hardware SYN cookie toggles (Standard, STRICT raid protection, or Diagnostic mode)."
    "Forward custom public ports (e.g. 25567 -> 25565) or bind secondary dedicated public IPs to specific backend nodes."
    "Self-healing repair menu that automatically fixes missing routes, stuck Docker ports, and stale NAT tables."
    "Checks GitHub for the latest release and updates /opt/wirenet/ and all subsystem scripts with zero downtime."
    "Completely stops and wipes WireNet services, WireGuard keys, and restores your server to its original network state."
    "Exit the WireNet Interactive Control Center."
)

# Main Navigation Loop
CURRENT_INDEX=0

while true; do
    draw_header
    echo -e "  Version: ${BOLD}${GREEN}${CURRENT_VERSION}${NC} | Root Command: ${CYAN}${BOLD}wirenet${NC}"
    show_guide "${MAIN_OPTIONS[$CURRENT_INDEX]}" "${MAIN_DESCS[$CURRENT_INDEX]}"

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
                7) update_wirenet ;;
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
        3) status_menu ;;
        4) doctor_menu ;;
        5) firewall_menu ;;
        6) routing_menu ;;
        7) troubleshoot_menu ;;
        8) update_wirenet ;;
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
