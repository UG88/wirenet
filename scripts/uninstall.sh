#!/usr/bin/env bash
# ==============================================================================
# Complete WireNet 100% Deep Uninstaller & System Cleaner
# Developed by UG88 | https://github.com/UG88/wirenet
# Completely wipes all WireNet services, binaries, configs, keys, and firewall rules
# ==============================================================================

set -euo pipefail

echo "=========================================================="
echo " 🧹 WireNet 100% Complete Uninstaller & Deep Cleaner"
echo "=========================================================="

if [[ $EUID -ne 0 ]]; then
   echo "[-] Error: This script must be run as root (or with sudo)." >&2
   exit 1
fi

# 1. Stop and disable all WireNet systemd services
echo "[1/7] Stopping and disabling all WireNet background services..."
systemctl stop wirenet-gateway.service wirenet-node.service wirenet-watcher.service wg-quick@wg0.service rinetd.service haproxy.service 2>/dev/null || true
systemctl disable wirenet-gateway.service wirenet-node.service wirenet-watcher.service wg-quick@wg0.service rinetd.service haproxy.service 2>/dev/null || true
wg-quick down wg0 2>/dev/null || true

# 2. Remove systemd service unit files
echo "[2/7] Removing systemd service definitions..."
rm -f /etc/systemd/system/wirenet-gateway.service
rm -f /etc/systemd/system/wirenet-node.service
rm -f /etc/systemd/system/wirenet-watcher.service
systemctl daemon-reload
systemctl reset-failed 2>/dev/null || true

# 3. Clean up all custom Policy Routing tables and rules
echo "[3/7] Cleaning up policy routing and kernel tables..."
ip rule del fwmark 0x1 table 100 2>/dev/null || true
ip rule del fwmark 0x1 table 200 2>/dev/null || true
ip rule del from 10.200.0.2 table 200 2>/dev/null || true
ip route flush table 100 2>/dev/null || true
ip route flush table 200 2>/dev/null || true
ip route del 10.200.0.0/24 dev wg0 2>/dev/null || true

# 4. Clean up all IPTables chains & filter tables
echo "[4/7] Cleaning up firewall and NAT tables..."
DEFAULT_IFACE=$(ip route show default 2>/dev/null | awk '{print $5}' | head -n1 || echo "eth0")

iptables -D INPUT -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25700,30000:40000 -j MC_TCP_FILTER 2>/dev/null || true
iptables -D INPUT -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j MC_UDP_FILTER 2>/dev/null || true
iptables -D FORWARD -i "$DEFAULT_IFACE" -p tcp -m multiport --dports 25565:25700,30000:40000 -j MC_TCP_FILTER 2>/dev/null || true
iptables -D FORWARD -i "$DEFAULT_IFACE" -p udp -m multiport --dports 25565:25700,19132:19140,24454,30000:40000 -j MC_UDP_FILTER 2>/dev/null || true

iptables -F MC_TCP_FILTER 2>/dev/null || true
iptables -X MC_TCP_FILTER 2>/dev/null || true
iptables -F MC_UDP_FILTER 2>/dev/null || true
iptables -X MC_UDP_FILTER 2>/dev/null || true

iptables -t nat -D POSTROUTING -o wg0 -j MASQUERADE 2>/dev/null || true
iptables -t nat -D POSTROUTING -o "$DEFAULT_IFACE" -j MASQUERADE 2>/dev/null || true
iptables -t nat -F PREROUTING 2>/dev/null || true
iptables -t mangle -F 2>/dev/null || true

# 5. Remove WireGuard configuration and cryptographic keys
echo "[5/7] Removing /etc/wireguard directory, keys, and sysctl configs..."
rm -rf /etc/wireguard
rm -f /etc/sysctl.d/99-wirenet.conf /etc/sysctl.d/98-minecraft-security.conf
rm -f /etc/rinetd.conf

# 6. Remove WireNet System Directories
echo "[6/7] Removing /opt/wirenet directory..."
rm -rf /opt/wirenet

# 7. Remove Global CLI Binaries and Commands
echo "[7/7] Removing global 'wirenet' and 'wirenet-daemon' commands..."
rm -f /usr/local/bin/wirenet
rm -f /usr/local/bin/wirenet-daemon

echo ""
echo "=========================================================="
echo " [✓] WireNet has been 100% completely and cleanly wiped!"
echo " All global commands, services, and configs are removed."
echo " Your server is restored to its original network state."
echo "=========================================================="
