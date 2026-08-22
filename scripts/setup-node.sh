#!/usr/bin/env bash
# ==============================================================================
# WireNet Pterodactyl Node Setup Script (Interactive Node Selector + Port Bridge)
# Works out of the box for ALL newly created servers with 0 manual intervention
# ==============================================================================

set -euo pipefail

echo "=========================================================="
echo " Starting WireNet Node Setup (Pterodactyl Node)"
echo "=========================================================="

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

# 1. Clean broken repos & install networking tools & rinetd
echo "[+] Installing WireGuard, networking tools, and rinetd port bridge..."
if command -v apt-get >/dev/null 2>&1; then
    export DEBIAN_FRONTEND=noninteractive
    rm -f /etc/apt/sources.list.d/*kali*.list /etc/apt/sources.list.d/*rolling*.list 2>/dev/null || true
    if [[ -f /etc/apt/sources.list ]]; then
        sed -i '/kali\.org/d' /etc/apt/sources.list 2>/dev/null || true
        sed -i '/kali\.download/d' /etc/apt/sources.list 2>/dev/null || true
    fi
    apt-get update -qq || true
    apt-get install -y -qq wireguard wireguard-tools iproute2 iptables rinetd curl ufw || apt-get install -y wireguard wireguard-tools iproute2 iptables rinetd curl ufw
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y wireguard-tools iproute iptables rinetd curl >/dev/null 2>&1 || true
elif command -v yum >/dev/null 2>&1; then
    yum install -y epel-release >/dev/null 2>&1 || true
    yum install -y wireguard-tools iproute iptables rinetd curl >/dev/null 2>&1 || true
fi

# 2. Kernel optimizations & packet forwarding
echo "[+] Enabling kernel packet forwarding..."
cat << 'EOF' > /etc/sysctl.d/99-wirenet.conf
net.ipv4.ip_forward = 1
net.ipv4.conf.all.forwarding = 1
net.ipv4.conf.all.route_localnet = 1
net.ipv4.conf.all.rp_filter = 2
net.ipv4.conf.default.rp_filter = 2
EOF
sysctl --system >/dev/null 2>&1 || sysctl -w net.ipv4.ip_forward=1 net.ipv4.conf.all.route_localnet=1 net.ipv4.conf.all.rp_filter=2

# 3. Detect Node Public IP
PRIMARY_IP=$(ip route get 1.1.1.1 2>/dev/null | awk '{print $7; exit}' || hostname -I 2>/dev/null | awk '{print $1}' || curl -s -4 --connect-timeout 2 -m 2 ifconfig.me 2>/dev/null || echo "127.0.0.1")
echo "[+] Detected Node Host IP: $PRIMARY_IP"

# 4. Interactive Node Number Selector
NODE_NUMBER="${NODE_NUMBER:-}"
NODE_IP="${NODE_IP:-}"

if [[ -z "$NODE_IP" && -z "$NODE_NUMBER" ]]; then
    echo ""
    echo "=========================================================="
    echo " Select Node Identity for this VPS:"
    echo "  [1] Node 1 (IP: 10.200.0.2) [Default]"
    echo "  [2] Node 2 (IP: 10.200.0.3)"
    echo "  [3] Node 3 (IP: 10.200.0.4)"
    echo "  [4] Node 4 (IP: 10.200.0.5)"
    echo "  [5] Node 5 (IP: 10.200.0.6)"
    echo "  [c] Custom IP (e.g. 10.200.0.10)"
    echo "=========================================================="
    
    if [[ -e /dev/tty ]]; then
        read -r -p "Enter Node Number [1-5 or c] (Default: 1): " NODE_CHOICE </dev/tty || NODE_CHOICE="1"
    else
        read -r -p "Enter Node Number [1-5 or c] (Default: 1): " NODE_CHOICE || NODE_CHOICE="1"
    fi
    
    NODE_CHOICE="${NODE_CHOICE:-1}"
    
    case "$NODE_CHOICE" in
        1) NODE_IP="10.200.0.2" ;;
        2) NODE_IP="10.200.0.3" ;;
        3) NODE_IP="10.200.0.4" ;;
        4) NODE_IP="10.200.0.5" ;;
        5) NODE_IP="10.200.0.6" ;;
        c|C)
            if [[ -e /dev/tty ]]; then
                read -r -p "Enter Custom WireGuard IP (e.g. 10.200.0.10): " NODE_IP </dev/tty
            else
                read -r -p "Enter Custom WireGuard IP (e.g. 10.200.0.10): " NODE_IP
            fi
            ;;
        *)
            if [[ "$NODE_CHOICE" =~ ^10\.200\.0\.[0-9]+$ ]]; then
                NODE_IP="$NODE_CHOICE"
            elif [[ "$NODE_CHOICE" =~ ^[0-9]+$ ]]; then
                NODE_OCTET=$((NODE_CHOICE + 1))
                NODE_IP="10.200.0.${NODE_OCTET}"
            else
                NODE_IP="10.200.0.2"
            fi
            ;;
    esac
elif [[ -n "$NODE_NUMBER" ]]; then
    NODE_OCTET=$((NODE_NUMBER + 1))
    NODE_IP="10.200.0.${NODE_OCTET}"
fi

NODE_IP="${NODE_IP:-10.200.0.2}"
echo "[+] Configured Node Virtual IP: $NODE_IP"

# 5. Interactive prompt for Gateway credentials
GW_ENDPOINT="${GW_ENDPOINT:-}"
GW_PUBLIC_KEY="${GW_PUBLIC_KEY:-}"

if [[ -z "$GW_ENDPOINT" ]]; then
    if [[ -e /dev/tty ]]; then
        read -r -p "Enter Gateway Public IP (e.g. 3.108.55.144): " GW_ENDPOINT </dev/tty
    else
        read -r -p "Enter Gateway Public IP (e.g. 3.108.55.144): " GW_ENDPOINT
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

# 6. Generate Node Cryptographic Keys
mkdir -p /etc/wireguard
chmod 700 /etc/wireguard

if [[ ! -f /etc/wireguard/node_private.key ]]; then
    echo "[+] Generating Node cryptographic keypair..."
    wg genkey | tee /etc/wireguard/node_private.key | wg pubkey > /etc/wireguard/node_public.key
    chmod 600 /etc/wireguard/node_private.key
fi

NODE_PRIVATE_KEY=$(cat /etc/wireguard/node_private.key)
NODE_PUBLIC_KEY=$(cat /etc/wireguard/node_public.key)

# 7. Create WireGuard Configuration with Transparent Real IP Policy Routing
cat << EOF > /etc/wireguard/wg0.conf
[Interface]
Address = $NODE_IP/24
PrivateKey = $NODE_PRIVATE_KEY
Table = off

# Transparent Real IP Return Routing via CONNMARK & Policy Routing
PostUp = iptables -A FORWARD -i %i -j ACCEPT; iptables -A FORWARD -o %i -j ACCEPT
PostUp = iptables -I INPUT 1 -i %i -j ACCEPT 2>/dev/null || true
PostUp = ip route add 10.200.0.0/24 dev %i 2>/dev/null || true
PostUp = iptables -t mangle -A PREROUTING -i %i -m conntrack --ctstate NEW -j CONNMARK --set-mark 0x1
PostUp = iptables -t mangle -A PREROUTING -j CONNMARK --restore-mark
PostUp = iptables -t mangle -A OUTPUT -j CONNMARK --restore-mark
PostUp = ip rule add fwmark 0x1 table 100 2>/dev/null || true
PostUp = ip route add default via 10.200.0.1 dev %i table 100 2>/dev/null || true

PostDown = iptables -D FORWARD -i %i -j ACCEPT 2>/dev/null || true
PostDown = iptables -D FORWARD -o %i -j ACCEPT 2>/dev/null || true
PostDown = iptables -D INPUT -i %i -j ACCEPT 2>/dev/null || true
PostDown = iptables -t mangle -D PREROUTING -i %i -m conntrack --ctstate NEW -j CONNMARK --set-mark 0x1 2>/dev/null || true
PostDown = iptables -t mangle -D PREROUTING -j CONNMARK --restore-mark 2>/dev/null || true
PostDown = iptables -t mangle -D OUTPUT -j CONNMARK --restore-mark 2>/dev/null || true
PostDown = ip rule del fwmark 0x1 table 100 2>/dev/null || true
PostDown = ip route flush table 100 2>/dev/null || true

[Peer]
PublicKey = $GW_PUBLIC_KEY
Endpoint = $GW_ENDPOINT:51820
AllowedIPs = 0.0.0.0/0
PersistentKeepalive = 25
EOF

# 8. Configure Automated rinetd Bridge for all Minecraft / Game Ports
echo "[+] Configuring automated rinetd port bridge for game server ports..."
mkdir -p /etc
cat << EOF > /etc/rinetd.conf
# WireNet Automated Docker Port Bridge (Node IP: $NODE_IP)
# Format: bindaddress bindport connectaddress connectport

# Minecraft Standard Port
$NODE_IP 25565 $PRIMARY_IP 25565
EOF

# Append port ranges 25566-25600 and 30000-30050
for port in $(seq 25566 25600); do
    echo "$NODE_IP $port $PRIMARY_IP $port" >> /etc/rinetd.conf
done

for port in $(seq 30000 30050); do
    echo "$NODE_IP $port $PRIMARY_IP $port" >> /etc/rinetd.conf
done

# 9. Clean conflicting iptables PREROUTING rules and shield backend Node from direct public access
iptables -t nat -F PREROUTING 2>/dev/null || true
iptables -t mangle -F 2>/dev/null || true

# Block direct internet access on eth0 to game ports so backend IP is 100% HIDDEN & INVISIBLE
DEFAULT_IFACE=$(ip route show default 2>/dev/null | awk '{print $5}' | head -n1 || echo "eth0")
iptables -D INPUT -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25700,30000:40000 -j DROP 2>/dev/null || true
iptables -I INPUT 1 -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25700,30000:40000 -j DROP 2>/dev/null || true

iptables -D INPUT -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DROP 2>/dev/null || true
iptables -I INPUT 1 -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j DROP 2>/dev/null || true

# Ensure Docker containers can reach Outbound Internet (Mojang Auth & DNS)
iptables -t nat -D POSTROUTING -s 172.16.0.0/12 -j MASQUERADE 2>/dev/null || true
iptables -t nat -A POSTROUTING -s 172.16.0.0/12 -j MASQUERADE 2>/dev/null || true
iptables -t nat -D POSTROUTING -s 10.200.0.0/24 -j MASQUERADE 2>/dev/null || true
iptables -t nat -A POSTROUTING -s 10.200.0.0/24 -j MASQUERADE 2>/dev/null || true

# Configure Docker public DNS and live-restore (never stops running containers)
mkdir -p /etc/docker
if [[ ! -f /etc/docker/daemon.json ]] || ! grep -q "dns" /etc/docker/daemon.json 2>/dev/null; then
    cat << 'EOF' > /etc/docker/daemon.json
{
  "dns": ["1.1.1.1", "8.8.8.8"],
  "live-restore": true
}
EOF
    systemctl reload docker 2>/dev/null || true
fi

# 10. Start and Enable Services
systemctl enable --now wg-quick@wg0
systemctl restart wg-quick@wg0

systemctl enable --now rinetd 2>/dev/null || true
systemctl restart rinetd 2>/dev/null || true

# 11. Install and Start WireNet Dynamic Port Watcher Daemon
echo "[+] Installing WireNet Dynamic Port Watcher Daemon..."
mkdir -p /opt/wirenet/scripts
curl -fsSL --max-time 5 -H "Cache-Control: no-cache" "https://raw.githubusercontent.com/UG88/wirenet/main/scripts/wirenet-watcher.sh?$(date +%s)" -o /opt/wirenet/scripts/wirenet-watcher.sh 2>/dev/null || true
chmod 755 /opt/wirenet/scripts/wirenet-watcher.sh 2>/dev/null || true

cat << 'EOF' > /etc/systemd/system/wirenet-watcher.service
[Unit]
Description=WireNet Dynamic Docker Port Watcher Daemon
After=docker.service wg-quick@wg0.service rinetd.service
Wants=docker.service

[Service]
Type=simple
ExecStart=/bin/bash /opt/wirenet/scripts/wirenet-watcher.sh
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload 2>/dev/null || true
systemctl enable wirenet-watcher.service 2>/dev/null || true
systemctl restart --no-block wirenet-watcher.service 2>/dev/null || true

echo "=========================================================="
echo " [✓] WireNet Node is ACTIVE & FULLY AUTOMATED! (Node IP: $NODE_IP)"
echo " Dynamic Port Watcher is running in background (auto-bridges any new server)!"
echo "=========================================================="
echo ""
echo " FINAL STEP: Run this SINGLE command on your Gateway VPS"
echo " to authorize this Node:"
echo ""
echo " sudo wg set wg0 peer $NODE_PUBLIC_KEY allowed-ips $NODE_IP/32"
echo " sudo ip route add $NODE_IP dev wg0 2>/dev/null || true"
echo "=========================================================="
