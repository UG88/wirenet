#!/usr/bin/env bash
# ==============================================================================
# WireNet Rust Daemon 1-Click Builder & Systemd Installer
# Developed by UG88 | https://github.com/UG88/wirenet
# Installs Rust, compiles wirenet-daemon, and configures systemd service
# ==============================================================================

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

echo "=========================================================="
echo " 🦀 WireNet Rust Daemon Builder & Installer"
echo "=========================================================="

# 1. Install Build Dependencies
echo "[1/5] Checking Build Dependencies (build-essential, curl, git)..."
apt-get update -qq && apt-get install -y -qq build-essential curl git pkg-config libssl-dev

# 2. Install Rust Toolchain if not present
if ! command -v cargo >/dev/null 2>&1 && [[ ! -f "$HOME/.cargo/bin/cargo" ]]; then
    echo "[2/5] Installing Rust & Cargo toolchain..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env" 2>/dev/null || export PATH="$HOME/.cargo/bin:$PATH"
else
    echo "[2/5] Rust Toolchain is already installed."
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# 3. Clone / Update WireNet Repo
echo "[3/5] Pulling latest WireNet Daemon source code..."
mkdir -p /opt/wirenet
if [[ ! -d /opt/wirenet/.git ]]; then
    rm -rf /tmp/wirenet_clone
    git clone https://github.com/UG88/wirenet.git /tmp/wirenet_clone
    cp -rf /tmp/wirenet_clone/* /opt/wirenet/ 2>/dev/null || true
    cp -rf /tmp/wirenet_clone/.git /opt/wirenet/ 2>/dev/null || true
    rm -rf /tmp/wirenet_clone
else
    cd /opt/wirenet && git fetch --all -q && git reset --hard origin/main -q
fi

# 4. Build Optimized Release Binary
echo "[4/5] Compiling High-Performance wirenet-daemon (Release Mode)..."
source "$HOME/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.cargo/bin:$PATH"
cd /opt/wirenet/daemon
cargo build --release

# Install binary globally
cp -f /opt/wirenet/daemon/target/release/wirenet-daemon /usr/local/bin/wirenet-daemon
chmod +x /usr/local/bin/wirenet-daemon
echo "  [✓] Binary installed to /usr/local/bin/wirenet-daemon"

# 5. Detect Role & Configure Systemd Service
echo "[5/5] Configuring systemd service..."

if [[ -f /etc/wireguard/gateway_private.key ]] || ip addr show dev wg0 2>/dev/null | grep -q "10.200.0.1/"; then
    # Gateway Service
    cat << 'EOF' > /etc/systemd/system/wirenet-gateway.service
[Unit]
Description=WireNet Rust Gateway Ingress & Anti-DDoS Daemon
After=network.target wg-quick@wg0.service
Wants=wg-quick@wg0.service

[Service]
Type=simple
ExecStart=/usr/local/bin/wirenet-daemon gateway run
Restart=always
RestartSec=3
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable --now wirenet-gateway.service
    echo "  [✓] wirenet-gateway.service is ACTIVE and running on Gateway!"

else
    # Node Service
    cat << 'EOF' > /etc/systemd/system/wirenet-node.service
[Unit]
Description=WireNet Rust Node Agent & Docker Auto-Discovery Daemon
After=network.target docker.service wg-quick@wg0.service
Wants=docker.service wg-quick@wg0.service

[Service]
Type=simple
ExecStart=/usr/local/bin/wirenet-daemon node run
Restart=always
RestartSec=3
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable --now wirenet-node.service
    echo "  [✓] wirenet-node.service is ACTIVE and running on Node!"
fi

echo ""
echo "=========================================================="
echo " 🎉 WireNet Rust Daemon Successfully Installed!"
echo "=========================================================="
echo " - View Live Dashboard : wirenet-daemon tui"
echo " - Check Doctor Health : wirenet-daemon doctor"
echo " - View Service Status : systemctl status wirenet-*.service"
echo "=========================================================="
