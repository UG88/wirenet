#!/usr/bin/env bash
# ==============================================================================
# WireNet Unified Rust Engine Installer (by UG88)
# Installs Rust toolchain, compiles native 'wirenet' CLI, and registers globally
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
echo "      🦀  WireNet Pure Rust Engine Global Installer       "
echo "=========================================================="
echo -e "${NC}"

# 1. Install system prerequisites
echo "[1/4] Installing system build prerequisites..."
if command -v apt-get >/dev/null 2>&1; then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq || true
    apt-get install -y -qq build-essential curl git pkg-config libssl-dev wireguard wireguard-tools iptables || true
fi

# 2. Check/Install Rust toolchain
echo "[2/4] Configuring Rust & Cargo toolchain..."
source "$HOME/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.cargo/bin:$PATH"

if ! command -v rustup >/dev/null 2>&1; then
    echo "  [+] Installing Rust toolchain..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    source "$HOME/.cargo/env" 2>/dev/null || true
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# 3. Download Source & Compile Release Binary
echo "[3/4] Downloading latest WireNet source & compiling..."
BUILD_DIR="/tmp/wirenet_build"
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"

git clone --depth 1 https://github.com/UG88/wirenet.git "${BUILD_DIR}" 2>/dev/null || {
    curl -fsSL https://github.com/UG88/wirenet/archive/refs/heads/main.tar.gz | tar -xz -C /tmp/
    mv /tmp/wirenet-main "${BUILD_DIR}"
}

cd "${BUILD_DIR}/daemon"
"$HOME/.cargo/bin/cargo" build --release

# 4. Install Global Binary
echo "[4/4] Installing binary to /usr/local/bin/wirenet..."
cp -f "${BUILD_DIR}/daemon/target/release/wirenet-daemon" /usr/local/bin/wirenet
chmod +x /usr/local/bin/wirenet
rm -rf "${BUILD_DIR}"

echo -e "${GREEN}${BOLD}"
echo "=========================================================="
echo " [✓] WireNet Unified Rust Engine Installed Successfully!"
echo " You can now run 'wirenet' from any terminal!"
echo "=========================================================="
echo -e "${NC}"

# Display Help / Usage
/usr/local/bin/wirenet --help
