# 🛠️ WireNet Troubleshooting & Error Fix Guide

Complete diagnostic reference and 1-click self-healing repair tools for WireNet Gateway and Pterodactyl Node networks.

---

## ⚡ 1-Command Universal Auto-Fix
Run this single command on **either your Gateway VPS or Node VPS** — it automatically detects the server role, runs 5 health checks, and repairs any broken routes or port bridges:

```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/scripts/troubleshoot.sh | sudo bash
```

---

## 🔍 Diagnostic Cheatsheet

| Test | Run On | Command | Expected Output |
| :--- | :--- | :--- | :--- |
| **Tunnel Status** | Gateway & Node | `sudo wg show` | Active peer with `latest handshake: seconds ago` |
| **Tunnel Ping** | Node | `ping -c 2 10.200.0.1` | `0% packet loss`, `< 15ms` latency |
| **Tunnel Ping** | Gateway | `ping -c 2 10.200.0.2` | `0% packet loss`, `< 15ms` latency |
| **Port Bridge** | Node | `nc -vz 10.200.0.2 25565` | `port [tcp/*] succeeded!` |
| **Gateway Reach** | Gateway | `nc -vz 10.200.0.2 25565` | `port [tcp/*] succeeded!` |

---

## 🚨 Common Issues & 1-Minute Fixes

### 1. `Connection Refused` on Port 25565
**Cause**: The Minecraft server is running on the Node, but Docker bound only to the Node's public IP (`139.59.8.66`), ignoring the WireGuard tunnel IP (`10.200.0.2`).

**Solution**:
Run the auto-repair tool on your **Node VPS**:
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/scripts/troubleshoot-node.sh | sudo bash
```
*(This cleans conflicting NAT tables and ensures the `rinetd` port bridge forwards `10.200.0.2:25565` directly to Docker)*.

---

### 2. `Connection Timed Out / Hanging`
**Cause**: AWS EC2 Security Group is blocking incoming game ports, or WireGuard handshake expired.

**Solutions**:
1. **In AWS Console**: Go to **EC2 ──► Instances ──► Security Groups ──► Edit Inbound Rules**:
   - Add **Custom TCP** | Port `0-65535` (or `25565-25600, 30000-40000`) | Source `0.0.0.0/0`
   - Add **Custom UDP** | Port `0-65535` (or `25565-25600, 30000-40000`) | Source `0.0.0.0/0`
2. **On Gateway VPS**: Run the Gateway auto-repair:
   ```bash
   curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/scripts/troubleshoot-gateway.sh | sudo bash
   ```

---

### 3. Server Console Displays `172.18.0.1` Instead of Real Player IP
**Cause**: Minecraft running inside Docker sees Docker's internal bridge gateway IP.

**Solution (PROXY Protocol v2)**:
1. **On Gateway VPS**: Install HAProxy with PROXY Protocol v2:
   ```bash
   sudo apt-get update -qq && sudo apt-get install -y haproxy
   cat << 'EOF' | sudo tee /etc/haproxy/haproxy.cfg
   global
       maxconn 10000
   defaults
       mode tcp
       timeout connect 5s
       timeout client 30m
       timeout server 30m
   listen minecraft
       bind 0.0.0.0:25565
       server node1 10.200.0.2:25565 send-proxy-v2
   EOF
   sudo systemctl restart haproxy
   ```
2. **In Minecraft Server**: Drop **[ProxyProtocol.jar](https://github.com/LOOHP/ProxyProtocol/releases/latest/download/ProxyProtocol.jar)** into your `plugins/` folder and restart your server.
3. Your server console will now log genuine player public IPs (e.g. `[104.28.228.88:52389] logged in`).

---

### 4. Adding a Second Node (Node 2, Node 3...)
When creating additional Pterodactyl nodes:

1. **On the New Node VPS** (assigning `10.200.0.3`):
   ```bash
   NODE_IP="10.200.0.3" curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/scripts/setup-node.sh | sudo bash
   ```
2. **On your Gateway VPS**, authorize Node 2:
   ```bash
   sudo wg set wg0 peer <NODE2_PUBLIC_KEY> allowed-ips 10.200.0.3/32
   sudo ip route add 10.200.0.3 dev wg0 2>/dev/null || true
   ```
3. Any server created on Node 2 in Pterodactyl on ports `25565-25600` or `30000-40050` connects immediately!

---

### 5. Custom Port Translation (e.g. Gateway Port 25567 ──► Node Port 25565)
To route a specific Gateway public port to a different port or different node:

```bash
# On Gateway VPS:
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/scripts/forward-port.sh | sudo bash -s -- 25567 10.200.0.3 25565
```
