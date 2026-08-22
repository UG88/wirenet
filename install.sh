#!/usr/bin/env bash
# ==============================================================================
# WireNet Global Installer & CLI Setup
# Developed by UG88 | https://github.com/UG88/wirenet
# Installs WireNet globally into /opt/wirenet and registers 'wirenet' command
# ==============================================================================

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}[-] Error: WireNet installer must be run as root (or with sudo).${NC}" >&2
   exit 1
fi

echo -e "${CYAN}${BOLD}"
echo "=========================================================="
echo "          🚀  Installing WireNet CLI (by UG88)            "
echo "=========================================================="
echo -e "${NC}"

INSTALL_DIR="/opt/wirenet"
BIN_PATH="/usr/local/bin/wirenet"
REPO_BASE="https://raw.githubusercontent.com/UG88/wirenet/main"

echo "[+] Creating WireNet system directory: ${INSTALL_DIR}..."
mkdir -p "${INSTALL_DIR}/scripts"
chmod 755 "${INSTALL_DIR}"

echo "[+] Downloading latest WireNet Master Manager and subsystems..."
curl -fsSL -H "Cache-Control: no-cache" "${REPO_BASE}/wirenet.sh?$(date +%s)" -o "${INSTALL_DIR}/wirenet.sh"
chmod 755 "${INSTALL_DIR}/wirenet.sh"

SCRIPTS=(
    "setup-gateway.sh"
    "setup-node.sh"
    "doctor.sh"
    "status.sh"
    "troubleshoot.sh"
    "troubleshoot-gateway.sh"
    "troubleshoot-node.sh"
    "forward-port.sh"
    "add-ip-mapping.sh"
    "firewall.sh"
    "fix-gateway.sh"
    "fix-node.sh"
    "fix-node-routing.sh"
    "uninstall.sh"
)

for script in "${SCRIPTS[@]}"; do
    curl -fsSL -H "Cache-Control: no-cache" "${REPO_BASE}/scripts/${script}?$(date +%s)" -o "${INSTALL_DIR}/scripts/${script}" 2>/dev/null || true
    chmod 755 "${INSTALL_DIR}/scripts/${script}" 2>/dev/null || true
done

# Save version stamp
echo "v1.2.0-$(date +%Y%m%d)" > "${INSTALL_DIR}/version"

# Create global command wrapper in /usr/local/bin/wirenet
echo "[+] Registering global 'wirenet' terminal command..."
cat << 'EOF' > "${BIN_PATH}"
#!/usr/bin/env bash
if [[ $EUID -ne 0 ]]; then
    exec sudo /opt/wirenet/wirenet.sh "$@"
else
    exec /opt/wirenet/wirenet.sh "$@"
fi
EOF
chmod 755 "${BIN_PATH}"

echo -e "${GREEN}${BOLD}"
echo "=========================================================="
echo " [✓] WireNet installed globally successfully!"
echo " You can now run 'wirenet' from ANY directory!"
echo "=========================================================="
echo -e "${NC}"

# Launch WireNet immediately
exec "${BIN_PATH}"
