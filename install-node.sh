#!/usr/bin/env bash
# ==============================================================================
# WireNet Dedicated Pterodactyl Node Installer (by UG88)
# Installs minimal 'wirenet-node' agent without database or dashboard overhead
# ==============================================================================

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}[-] Error: WireNet Node installer must be run as root (or with sudo).${NC}" >&2
   exit 1
fi

GATEWAY=""
GATEWAY_KEY=""
VIRTUAL_IP="10.200.0.2"

while [[ $# -gt 0 ]]; do
  case $1 in
    --gateway)
      GATEWAY="$2"
      shift 2
      ;;
    --gateway-key|-k)
      GATEWAY_KEY="$2"
      shift 2
      ;;
    --virtual-ip)
      VIRTUAL_IP="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

echo -e "${CYAN}${BOLD}"
echo "=========================================================="
echo "    🦀  WireNet Dedicated Pterodactyl Node Installer      "
echo "=========================================================="
echo -e "${NC}"

# 1. Install system prerequisites
echo "[1/4] Installing system network prerequisites..."
if command -v apt-get >/dev/null 2>&1; then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq || true
    apt-get install -y -qq build-essential curl git pkg-config libssl-dev wireguard wireguard-tools iptables || true
fi

# 2. Check/Install Rust toolchain
echo "[2/4] Configuring Rust toolchain..."
source "$HOME/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.cargo/bin:$PATH"

if ! command -v rustup >/dev/null 2>&1; then
    echo "  [+] Installing minimal Rust toolchain..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
    source "$HOME/.cargo/env" 2>/dev/null || true
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# 3. Download Source & Compile ONLY wirenet-node
echo "[3/4] Downloading source & building lightweight wirenet-node..."
BUILD_DIR="/tmp/wirenet_node_build"
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"

git clone --depth 1 https://github.com/UG88/wirenet.git "${BUILD_DIR}" 2>/dev/null || {
    curl -fsSL https://github.com/UG88/wirenet/archive/refs/heads/main.tar.gz | tar -xz -C /tmp/
    mv /tmp/wirenet-main "${BUILD_DIR}"
}

cd "${BUILD_DIR}/daemon"
"$HOME/.cargo/bin/cargo" build --release --bin wirenet-node

# 4. Install Global Binary
echo "[4/4] Installing binary to /usr/local/bin/wirenet-node..."
cp -f "${BUILD_DIR}/daemon/target/release/wirenet-node" /usr/local/bin/wirenet-node
chmod +x /usr/local/bin/wirenet-node
rm -rf "${BUILD_DIR}"

# Provide convenient wirenet alias on the node if wirenet is not present
if ! command -v wirenet >/dev/null 2>&1; then
    ln -sf /usr/local/bin/wirenet-node /usr/local/bin/wirenet
fi

echo -e "${GREEN}${BOLD}"
echo "=========================================================="
echo " [✓] WireNet Dedicated Node Agent Installed Successfully!"
echo "=========================================================="
echo -e "${NC}"

if [[ -n "$GATEWAY" && -n "$GATEWAY_KEY" ]]; then
    echo -e "${YELLOW}[+] Auto-configuring node with Gateway: ${GATEWAY}${NC}"
    /usr/local/bin/wirenet-node setup --gateway "$GATEWAY" --gateway-key "$GATEWAY_KEY" --virtual-ip "$VIRTUAL_IP"
    /usr/local/bin/wirenet-node install-service
else
    echo "To configure and link this node to your Gateway, run:"
    echo "  wirenet-node setup --gateway <GATEWAY_IP>:51820 --gateway-key \"<GATEWAY_KEY>\""
    echo ""
    echo "To start the background Docker auto-discovery service:"
    echo "  wirenet-node install-service"
    echo ""
    echo "To check node status anytime:"
    echo "  wirenet-node status"
    echo ""
fi
