# WireNet — Kernel-Level Minecraft Ingress & Anti-DDoS Shield for Pterodactyl

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![GitHub Repository](https://img.shields.io/badge/GitHub-UG88%2Fwirenet-blue.svg)](https://github.com/UG88/wirenet)

**WireNet** is a production-grade, kernel-level networking and anti-DDoS shield designed specifically for Minecraft and game server hosting providers operating on [Pterodactyl](https://pterodactyl.io).

It replaces userspace reverse proxies (like FRP) with native Linux kernel transparent tunneling (WireGuard) and packet filtering (`iptables`/`conntrack`), delivering:
- **100% Native Real Player IPs** — Zero plugins required on any server flavor (Vanilla, Fabric, Forge, NeoForge, Paper, Purpur, Spigot, Bedrock, Geyser).
- **100% Hidden Backend Node IPs** — Protects Pterodactyl node hardware, storage, and databases from direct internet scans and attacks.
- **Built-in Kernel Anti-DDoS & Packet Filtering Shield** — Hardware SYN cookies, malformed packet dropper, bot join throttler, and Bedrock UDP reflection shields.
- **Multi-Node & Multi-IP Scaling** — Route multiple dedicated public IPs to different backend nodes so every customer VM gets default port `25565`.
- **Zero-Downtime Dynamic Reconfiguration** — Toggle or upgrade firewall modes without disconnecting active players.
- **Split-Tunneling Safety** — Restricts tunnel routing to internal subnets so your SSH terminal session and panel connection are never interrupted.

---

## 1. Network Topology

```
[ Public Internet / Players (Player IP: 1.2.3.4) ]
                     │
                     │ Connects to Gateway Public IP: 3.108.50.20:25565
                     ▼
┌───────────────────────────────────────────────────────────────┐
│                 WireNet Gateway VPS (Hub)                     │
│  - Public IP: 3.108.50.20                                     │
│  - WireGuard Virtual IP: 10.200.0.1                           │
│  - Kernel Packet Filter & SYN Cookie Anti-DDoS Shield         │
│  - Kernel DNAT (No SNAT -> Source IP 1.2.3.4 preserved!)      │
└───────────────────────────────┬───────────────────────────────┘
                                │
                                │ Encrypted WireGuard Kernel Pipe (ChaCha20-Poly1305)
                                ▼
┌───────────────────────────────────────────────────────────────┐
│              Private Pterodactyl Node VPS (Spoke)             │
│  - Backend Public IP is 100% HIDDEN                           │
│  - WireGuard Virtual IP: 10.200.0.2                           │
│  - Split-Tunneling (AllowedIPs = 10.200.0.0/24)               │
│  - Policy Routing (table 200 return path via Gateway)         │
└───────────────────────────────┬───────────────────────────────┘
                                │
                                ▼
┌───────────────────────────────────────────────────────────────┐
│              Customer's Minecraft Server (Wings)              │
│  - Receives packet with Source IP = 1.2.3.4                   │
│  - Works for Vanilla, Forge, Fabric, Paper, Bedrock           │
│  - /ban-ip <player> safely bans ONLY that player!             │
└───────────────────────────────────────────────────────────────┘
```

---

## 2. Fast 3-Step Setup

### Step 1: Set Up the Gateway VPS (Run on Gateway VPS)

On your **Public Gateway VPS**, run:
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/setup-gateway.sh | sudo bash
```

**What this does automatically:**
1. Installs official `wireguard`, `wireguard-tools`, and `iptables`.
2. Applies kernel network tuning and cryptographic SYN cookie protections.
3. Configures the `10.200.0.1` WireGuard hub.
4. Sets up DNAT port forwarding for ports `25565–25600` and `30000–40000`.
5. Deploys the Minecraft Anti-DDoS and Packet Filtering Shield.
6. **Prints the Gateway Public IP and Gateway Public Key.** *(Save these for Step 2!)*

---

### Step 2: Set Up the Pterodactyl Node VPS (Run on Node VPS)

Replace `YOUR_GATEWAY_IP` and `YOUR_GATEWAY_PUBLIC_KEY` with the values printed in Step 1:

```bash
GW_ENDPOINT="YOUR_GATEWAY_IP" GW_PUBLIC_KEY="YOUR_GATEWAY_PUBLIC_KEY" curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/setup-node.sh | sudo bash
```

**What this does automatically:**
1. Installs `wireguard` and `iproute2`.
2. Generates node cryptographic keys.
3. Sets up `10.200.0.2` with split-tunneling (SSH connection is never interrupted).
4. Configures policy routing (table 200) for seamless return traffic.
5. **Outputs a single authorization command.**

---

### Step 3: Authorize Node on Gateway VPS (Run on Gateway VPS)

Paste the authorization command printed by Step 2 into your **Gateway VPS** terminal:
```bash
sudo wg set wg0 peer <NODE_PUBLIC_KEY> allowed-ips 10.200.0.2/32
```

---

### Step 4: Test & Verify Connection

On your **Pterodactyl Node VPS**, test the tunnel:
```bash
ping -c 3 10.200.0.1
```
*(Should return responses with `<1ms` ping!)*

---

## 3. Multi-Node & Multi-IP Scaling Guide

WireNet supports connecting **unlimited backend nodes** across single or multiple public IP addresses.

### Method A: Single Public IP with Port Ranges (Shared Gateway)
- **Node 1** (`10.200.0.2`): Assigned ports `30001–30500`
- **Node 2** (`10.200.0.3`): Assigned ports `30501–31000`
- **Node 3** (`10.200.0.4`): Assigned ports `31001–31500`

To add Node 2 or Node 3, simply specify `NODE_IP` during setup:
```bash
NODE_IP="10.200.0.3" GW_ENDPOINT="GATEWAY_IP" GW_PUBLIC_KEY="GATEWAY_KEY" curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/setup-node.sh | sudo bash
```

---

### Method B: Dedicated Public IP per Node (Every Server Gets Port `25565`!)

If you have multiple secondary Public IPs attached to your Gateway VPS:
- **Public IP 1** (`3.108.50.21`) $\longrightarrow$ **Node 1** (`10.200.0.2:25565`)
- **Public IP 2** (`3.108.50.22`) $\longrightarrow$ **Node 2** (`10.200.0.3:25565`)
- **Public IP 3** (`3.108.50.23`) $\longrightarrow$ **Node 3** (`10.200.0.4:25565`)

Map each Public IP to each Node in **1 command** on the Gateway VPS:
```bash
# Map Public IP 1 to Node 1
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/add-ip-mapping.sh) 3.108.50.21 10.200.0.2

# Map Public IP 2 to Node 2
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/add-ip-mapping.sh) 3.108.50.22 10.200.0.3

# Map Public IP 3 to Node 3
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/add-ip-mapping.sh) 3.108.50.23 10.200.0.4
```

---

## 4. Minecraft Anti-DDoS & Firewall Management

Manage the kernel packet filtering shield on your Gateway VPS at any time with **zero player disconnections**:

```bash
# View live attack statistics and dropped packet counters
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/firewall.sh) status

# Turn on standard hosting protection
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/firewall.sh) enable

# Turn on STRICT anti-bot mode (during heavy bot raids)
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/firewall.sh) strict

# Temporarily disable shield (zero player disconnects)
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/firewall.sh) disable
```

---

## 5. Locking Down Backend Nodes (Complete Isolation)

To ensure attackers cannot bypass the Gateway VPS to attack your backend node directly, lock down incoming game ports on your **Pterodactyl Node VPS**:

```bash
# Allow game traffic ONLY through the internal WireNet tunnel (wg0)
sudo ufw allow in on wg0 to any port 25565:25600 proto tcp
sudo ufw allow in on wg0 to any port 25565:25600 proto udp
sudo ufw allow in on wg0 to any port 30000:40000 proto tcp
sudo ufw allow in on wg0 to any port 30000:40000 proto udp

# Block direct public internet access to game ports on physical interface (eth0)
sudo ufw deny in on eth0 to any port 25565:25600
sudo ufw deny in on eth0 to any port 30000:40000
```

---

## 6. Script Summary Reference

| Script | Server Location | Purpose |
|---|---|---|
| [`setup-gateway.sh`](setup-gateway.sh) | **Gateway VPS** | Deploys WireGuard Hub, port forwards, and Anti-DDoS Shield |
| [`setup-node.sh`](setup-node.sh) | **Node VPS** | Connects Pterodactyl Node with split-tunneling & policy routing |
| [`add-ip-mapping.sh`](add-ip-mapping.sh) | **Gateway VPS** | Maps dedicated Public IPs to specific backend Nodes |
| [`firewall.sh`](firewall.sh) | **Gateway VPS** | Controls the dynamic firewall (`status`, `enable`, `strict`, `disable`) |
| [`uninstall.sh`](uninstall.sh) | **Any VPS** | Completely wipes WireNet services and configuration |

---

## 7. Clean Uninstallation

To completely stop and remove WireNet from any VPS:
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/uninstall.sh | sudo bash
```

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
