#!/usr/bin/env bash
# ==============================================================================
# WireNet Pterodactyl Node Setup Script (Transparent Spoke with Split-Tunneling)
# ==============================================================================

set -euo pipefail

echo "=========================================================="
echo " Starting WireNet Node Installation (Pterodactyl Node)"
echo "=========================================================="

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

# 1. Clean broken third-party repos & Install WireGuard and IPRoute2
echo "[+] Installing WireGuard and networking tools..."
if command -v apt-get >/dev/null 2>&1; then
    export DEBIAN_FRONTEND=noninteractive
    rm -f /etc/apt/sources.list.d/*kali*.list /etc/apt/sources.list.d/*rolling*.list 2>/dev/null || true
    if [[ -f /etc/apt/sources.list ]]; then
        sed -i '/kali\.org/d' /etc/apt/sources.list 2>/dev/null || true
        sed -i '/kali\.download/d' /etc/apt/sources.list 2>/dev/null || true
    fi
    apt-get update -qq || true
    apt-get install -y -qq wireguard wireguard-tools iproute2 curl || apt-get install -y wireguard wireguard-tools iproute2 curl
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y wireguard-tools iproute curl >/dev/null 2>&1 || true
elif command -v yum >/dev/null 2>&1; then
    yum install -y epel-release >/dev/null 2>&1 || true
    yum install -y wireguard-tools iproute curl >/dev/null 2>&1 || true
fi

# 2. Interactive prompt for Gateway credentials (reads from /dev/tty when piped)
GW_ENDPOINT="${GW_ENDPOINT:-}"
GW_PUBLIC_KEY="${GW_PUBLIC_KEY:-}"

if [[ -z "$GW_ENDPOINT" ]]; then
    if [[ -e /dev/tty ]]; then
        read -r -p "Enter Gateway Public IP (e.g. 3.108.50.20): " GW_ENDPOINT </dev/tty
    else
        read -r -p "Enter Gateway Public IP (e.g. 3.108.50.20): " GW_ENDPOINT
    fi
fi

if [[ -z "$GW_PUBLIC_KEY" ]]; then
    if [[ -e /dev/tty ]]; then
        read -r -p "Enter Gateway Public Key (from Gateway setup): " GW_PUBLIC_KEY </dev/tty
    else
        read -r -p "Enter Gateway Public Key (from Gateway setup): " GW_PUBLIC_KEY
    fi
fi

if [[ -z "$GW_ENDPOINT" || -z "$GW_PUBLIC_KEY" ]]; then
    echo "[-] Error: Gateway Public IP and Gateway Public Key cannot be empty!" >&2
    echo "    Please run: GW_ENDPOINT=<IP> GW_PUBLIC_KEY=<KEY> sudo bash setup-node.sh" >&2
    exit 1
fi

# 3. Generate Node Cryptographic Keys
mkdir -p /etc/wireguard
chmod 700 /etc/wireguard

if [[ ! -f /etc/wireguard/node_private.key ]]; then
    echo "[+] Generating Node cryptographic keypair..."
    wg genkey | tee /etc/wireguard/node_private.key | wg pubkey > /etc/wireguard/node_public.key
    chmod 600 /etc/wireguard/node_private.key
fi

NODE_IP="${NODE_IP:-10.200.0.2}"
NODE_PRIVATE_KEY=$(cat /etc/wireguard/node_private.key)
NODE_PUBLIC_KEY=$(cat /etc/wireguard/node_public.key)

# 4. Create Node WireGuard Configuration with Split-Tunneling & Policy Routing
cat << EOF > /etc/wireguard/wg0.conf
[Interface]
Address = $NODE_IP/24
PrivateKey = $NODE_PRIVATE_KEY

# Policy routing: Game response packets for incoming tunnel traffic route back through Gateway
PostUp = ip rule add from $NODE_IP table 200 || true; ip route add default via 10.200.0.1 dev %i table 200 || true
PostDown = ip rule del from $NODE_IP table 200 || true; ip route del default via 10.200.0.1 dev %i table 200 || true

[Peer]
PublicKey = $GW_PUBLIC_KEY
Endpoint = $GW_ENDPOINT:51820
AllowedIPs = 10.200.0.0/24
PersistentKeepalive = 25
EOF

# 5. Start WireGuard on Node
systemctl enable --now wg-quick@wg0
systemctl restart wg-quick@wg0

echo "=========================================================="
echo " [✓] WireNet Node tunnel is ACTIVE! (Node IP: $NODE_IP)"
echo "=========================================================="
echo ""
echo " FINAL STEP: Run this SINGLE command on your Gateway VPS"
echo " to authorize this Node:"
echo ""
echo " sudo wg set wg0 peer $NODE_PUBLIC_KEY allowed-ips $NODE_IP/32"
echo ""
echo "=========================================================="
