#!/usr/bin/env bash
# ==============================================================================
# WireNet Gateway Setup Script (Transparent Ingress Hub + Anti-DDoS Shield)
# ==============================================================================

set -euo pipefail

echo "=========================================================="
echo " Starting WireNet Gateway Installation (Transparent Hub)"
echo "=========================================================="

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

# 1. Clean broken third-party repos & Install WireGuard and Tools
echo "[+] Installing WireGuard and networking tools..."
if command -v apt-get >/dev/null 2>&1; then
    export DEBIAN_FRONTEND=noninteractive
    rm -f /etc/apt/sources.list.d/*kali*.list /etc/apt/sources.list.d/*rolling*.list 2>/dev/null || true
    if [[ -f /etc/apt/sources.list ]]; then
        sed -i '/kali\.org/d' /etc/apt/sources.list 2>/dev/null || true
        sed -i '/kali\.download/d' /etc/apt/sources.list 2>/dev/null || true
    fi
    apt-get update -qq || true
    apt-get install -y -qq wireguard wireguard-tools iptables ufw curl || apt-get install -y wireguard wireguard-tools iptables ufw curl
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y wireguard-tools iptables curl >/dev/null 2>&1 || true
elif command -v yum >/dev/null 2>&1; then
    yum install -y epel-release >/dev/null 2>&1 || true
    yum install -y wireguard-tools iptables curl >/dev/null 2>&1 || true
fi

# 2. Enable Kernel IP Forwarding & Security Tuning
echo "[+] Applying kernel packet forwarding and network optimizations..."
cat << 'EOF' > /etc/sysctl.d/99-wirenet.conf
net.ipv4.ip_forward = 1
net.ipv4.conf.all.forwarding = 1
net.ipv4.conf.all.rp_filter = 2
net.ipv4.conf.default.rp_filter = 2
net.ipv4.tcp_syncookies = 1
net.ipv4.tcp_max_syn_backlog = 65536
net.core.somaxconn = 65535
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_fin_timeout = 15
EOF
sysctl --system >/dev/null 2>&1 || sysctl -w net.ipv4.ip_forward=1 net.ipv4.conf.all.rp_filter=2

# 3. Detect Primary Network Interface
DEFAULT_IFACE=$(ip route show default 2>/dev/null | awk '{print $5}' | head -n1)
DEFAULT_IFACE="${DEFAULT_IFACE:-eth0}"
echo "[+] Detected primary public interface: $DEFAULT_IFACE"

# 4. Generate Gateway Cryptographic Keys
mkdir -p /etc/wireguard
chmod 700 /etc/wireguard

if [[ ! -f /etc/wireguard/gateway_private.key ]]; then
    echo "[+] Generating WireNet cryptographic keypair..."
    wg genkey | tee /etc/wireguard/gateway_private.key | wg pubkey > /etc/wireguard/gateway_public.key
    chmod 600 /etc/wireguard/gateway_private.key
fi

GW_PRIVATE_KEY=$(cat /etc/wireguard/gateway_private.key)
GW_PUBLIC_KEY=$(cat /etc/wireguard/gateway_public.key)
PUBLIC_IP=$(curl -s -4 ifconfig.me || curl -s -4 icanhazip.com || echo "YOUR_GATEWAY_PUBLIC_IP")

# 5. Create WireGuard Configuration
cat << EOF > /etc/wireguard/wg0.conf
[Interface]
Address = 10.200.0.1/24
ListenPort = 51820
PrivateKey = $GW_PRIVATE_KEY

# Forwarding Rules (DNAT without Masquerading -> Real Player IPs Preserved!)
PostUp = iptables -A FORWARD -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT
PostUp = iptables -A FORWARD -i %i -j ACCEPT; iptables -A FORWARD -o %i -j ACCEPT
PostUp = iptables -t nat -A PREROUTING -i $DEFAULT_IFACE -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination 10.200.0.2
PostUp = iptables -t nat -A PREROUTING -i $DEFAULT_IFACE -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DNAT --to-destination 10.200.0.2

PostDown = iptables -D FORWARD -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT 2>/dev/null || true
PostDown = iptables -D FORWARD -i %i -j ACCEPT 2>/dev/null || true; iptables -D FORWARD -o %i -j ACCEPT 2>/dev/null || true
PostDown = iptables -t nat -D PREROUTING -i $DEFAULT_IFACE -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination 10.200.0.2 2>/dev/null || true
PostDown = iptables -t nat -D PREROUTING -i $DEFAULT_IFACE -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DNAT --to-destination 10.200.0.2 2>/dev/null || true
EOF

# 6. Apply Minecraft Anti-DDoS Filter Chains
echo "[+] Initializing Minecraft Anti-DDoS and Packet Filtering Shield..."
iptables -N MC_TCP_FILTER 2>/dev/null || iptables -F MC_TCP_FILTER
iptables -N MC_UDP_FILTER 2>/dev/null || iptables -F MC_UDP_FILTER

iptables -A MC_TCP_FILTER -m state --state INVALID -j DROP
iptables -A MC_TCP_FILTER -p tcp --tcp-flags ALL NONE -j DROP
iptables -A MC_TCP_FILTER -p tcp --tcp-flags ALL ALL -j DROP
iptables -A MC_TCP_FILTER -p tcp --tcp-flags SYN,FIN SYN,FIN -j DROP
iptables -A MC_TCP_FILTER -p tcp --tcp-flags SYN,RST SYN,RST -j DROP
iptables -A MC_TCP_FILTER -p tcp --syn -m hashlimit --hashlimit-above 25/sec --hashlimit-burst 50 --hashlimit-mode srcip --hashlimit-name mc_tcp_limit -j DROP
iptables -A MC_TCP_FILTER -j ACCEPT

iptables -A MC_UDP_FILTER -p udp -m hashlimit --hashlimit-above 60/sec --hashlimit-burst 120 --hashlimit-mode srcip --hashlimit-name mc_udp_limit -j DROP
iptables -A MC_UDP_FILTER -j ACCEPT

iptables -D INPUT -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25600,30000:40000 -j MC_TCP_FILTER 2>/dev/null || true
iptables -A INPUT -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25600,30000:40000 -j MC_TCP_FILTER

iptables -D INPUT -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25600,30000:40000 -j MC_UDP_FILTER 2>/dev/null || true
iptables -A INPUT -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25600,30000:40000 -j MC_UDP_FILTER

# 7. Start & Enable WireGuard Service
systemctl enable --now wg-quick@wg0
systemctl restart wg-quick@wg0

echo "=========================================================="
echo " [✓] WireNet Gateway & Anti-DDoS Shield is ACTIVE!"
echo "=========================================================="
echo ""
echo " Save these Gateway details for setting up your Node:"
echo "   Gateway Public IP: $PUBLIC_IP"
echo "   Gateway Public Key: $GW_PUBLIC_KEY"
echo "   Gateway WireGuard Port: 51820"
echo "=========================================================="
