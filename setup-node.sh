#!/usr/bin/env bash
# ==============================================================================
# WireNet Node Setup Script (True Transparent Real-IP Routing with Table=off)
# Preserves 100% Real Player Public IPs into Minecraft Docker containers
# ==============================================================================

set -euo pipefail

echo "=========================================================="
echo " Starting WireNet Real-IP Node Setup (Pterodactyl Node)"
echo "=========================================================="

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

# 1. Clean broken repos & install networking tools
echo "[+] Installing WireGuard and networking tools..."
if command -v apt-get >/dev/null 2>&1; then
    export DEBIAN_FRONTEND=noninteractive
    rm -f /etc/apt/sources.list.d/*kali*.list /etc/apt/sources.list.d/*rolling*.list 2>/dev/null || true
    if [[ -f /etc/apt/sources.list ]]; then
        sed -i '/kali\.org/d' /etc/apt/sources.list 2>/dev/null || true
        sed -i '/kali\.download/d' /etc/apt/sources.list 2>/dev/null || true
    fi
    apt-get update -qq || true
    apt-get install -y -qq wireguard wireguard-tools iproute2 iptables curl || apt-get install -y wireguard wireguard-tools iproute2 iptables curl
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y wireguard-tools iproute iptables curl >/dev/null 2>&1 || true
elif command -v yum >/dev/null 2>&1; then
    yum install -y epel-release >/dev/null 2>&1 || true
    yum install -y wireguard-tools iproute iptables curl >/dev/null 2>&1 || true
fi

# 2. Kernel optimizations
echo "[+] Enabling packet forwarding and loose reverse path filter..."
cat << 'EOF' > /etc/sysctl.d/99-wirenet.conf
net.ipv4.ip_forward = 1
net.ipv4.conf.all.forwarding = 1
net.ipv4.conf.all.rp_filter = 2
net.ipv4.conf.default.rp_filter = 2
EOF
sysctl --system >/dev/null 2>&1 || sysctl -w net.ipv4.ip_forward=1 net.ipv4.conf.all.rp_filter=2

# 3. Interactive prompt for Gateway credentials
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
    exit 1
fi

# 4. Generate Node Cryptographic Keys
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

# 5. Create WireGuard config with Table=off and CONNMARK routing for Real-IP
cat << EOF > /etc/wireguard/wg0.conf
[Interface]
Address = $NODE_IP/24
PrivateKey = $NODE_PRIVATE_KEY
# Disable automatic default route hijacking so SSH never disconnects:
Table = off

# Add local subnet route:
PostUp = ip route add 10.200.0.0/24 dev %i table main 2>/dev/null || true

# Connection tracking policy routing for Real Player IPs:
PostUp = iptables -A FORWARD -i %i -j ACCEPT; iptables -A FORWARD -o %i -j ACCEPT
PostUp = iptables -t mangle -A PREROUTING -i %i -j CONNMARK --set-mark 0x200
PostUp = iptables -t mangle -A PREROUTING -m connmark --mark 0x200 -j CONNMARK --restore-mark
PostUp = iptables -t mangle -A OUTPUT -m connmark --mark 0x200 -j CONNMARK --restore-mark
PostUp = ip rule add fwmark 0x200 table 200 || true
PostUp = ip route add default via 10.200.0.1 dev %i table 200 || true

PostDown = ip route del 10.200.0.0/24 dev %i table main 2>/dev/null || true
PostDown = iptables -D FORWARD -i %i -j ACCEPT 2>/dev/null || true; iptables -D FORWARD -o %i -j ACCEPT 2>/dev/null || true
PostDown = iptables -t mangle -D PREROUTING -i %i -j CONNMARK --set-mark 0x200 2>/dev/null || true
PostDown = iptables -t mangle -D PREROUTING -m connmark --mark 0x200 -j CONNMARK --restore-mark 2>/dev/null || true
PostDown = iptables -t mangle -D OUTPUT -m connmark --mark 0x200 -j CONNMARK --restore-mark 2>/dev/null || true
PostDown = ip rule del fwmark 0x200 table 200 2>/dev/null || true; ip route del default via 10.200.0.1 dev %i table 200 2>/dev/null || true

[Peer]
PublicKey = $GW_PUBLIC_KEY
Endpoint = $GW_ENDPOINT:51820
# AllowedIPs=0.0.0.0/0 allows WireGuard to encrypt return packets to any real player IP:
AllowedIPs = 0.0.0.0/0
PersistentKeepalive = 25
EOF

# 6. Start WireGuard
systemctl enable --now wg-quick@wg0
systemctl restart wg-quick@wg0

echo "=========================================================="
echo " [✓] WireNet Node is ACTIVE with True Real-IP Forwarding!"
echo "=========================================================="
echo ""
echo " Authorize this Node on Gateway VPS:"
echo " sudo wg set wg0 peer $NODE_PUBLIC_KEY allowed-ips $NODE_IP/32"
echo "=========================================================="
