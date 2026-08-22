# WireNet — Kernel-Level Minecraft Ingress & Anti-DDoS Shield for Pterodactyl

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![GitHub Repository](https://img.shields.io/badge/GitHub-UG88%2Fwirenet-blue.svg)](https://github.com/UG88/wirenet)
[![Rust Engine](https://img.shields.io/badge/Engine-Async%20Tokio%20Rust-orange.svg)](daemon/)

**WireNet** is an enterprise, high-performance tunneling and Anti-DDoS protection system designed specifically for Minecraft and game hosting providers operating on [Pterodactyl](https://pterodactyl.io).

It replaces slow userspace reverse proxies (like FRP) with native Linux kernel WireGuard tunneling and a unified, memory-safe **Rust Daemon (`wirenet-daemon`)**, delivering:
- **100% Real Player IPs (Zero Plugins)** — True client home IPs passed directly to Vanilla, Paper, Purpur, Velocity, Fabric, Forge, and Geyser at the Linux kernel layer.
- **100% Hidden Backend Node IPs** — Complete shield preventing attackers and port scanners from ever finding or attacking your backend Pterodactyl node VPS.
- **Kernel & Rust Anti-DDoS Scrubbing** — Hardware SYN cookies, per-IP token bucket rate limiting, and bot raid mitigation.
- **0ms Docker Auto-Discovery** — Detects container start/stop events directly from `/var/run/docker.sock` in real-time.
- **Zero-Flicker Live Streaming TUI** — Real-time rolling packet sparklines, load capacity gauges, and live player IP sniffer (`ratatui` + `crossterm`).
- **Interactive TUI Control Center** — Unified `wirenet` command with arrow-key menu navigation and 1-click self-healing doctor.

---

## 🗺️ Network Topology

```
[ Public Internet / Players (Real IP: 104.28.228.88) ]
                     │
                     │ Connects to Gateway Public IP: 3.108.55.144:25565
                     ▼
┌───────────────────────────────────────────────────────────────┐
│                 WireNet Gateway VPS (Hub)                     │
│  - Public IPv4: 3.108.55.144                                  │
│  - Private Virtual IP: 10.200.0.1                             │
│  - Anti-DDoS Scrubbing Shield (SYN Floods & Bot Mitigator)    │
│  - Pure Transparent Layer-3 DNAT (Preserves Real Client IP)   │
│  - Asynchronous Tokio Control Plane (:9000)                   │
└───────────────────────────────┬───────────────────────────────┘
                                │
                                │ Encrypted WireGuard Kernel Fastpath (0.8ms RTT)
                                ▼
┌───────────────────────────────────────────────────────────────┐
│              Private Pterodactyl Node VPS (Spoke)             │
│  - Backend Public IP is 100% HIDDEN & INVISIBLE               │
│  - Private Virtual IP: 10.200.0.2                             │
│  - Docker /docker.sock Auto-Discovery (0ms port sync)         │
│  - Policy Routing Table 100 + CONNMARK (Symmetric Returns)    │
└───────────────────────────────┬───────────────────────────────┘
                                │
                                ▼
┌───────────────────────────────────────────────────────────────┐
│              Customer's Minecraft Server (Docker)             │
│  - Receives genuine player IP: 104.28.228.88 (ZERO PLUGINS!)  │
│  - Full support for /ban-ip, Geolocation, and Auth            │
│  - Vanilla, Paper, Purpur, Velocity, Fabric, Bedrock          │
└───────────────────────────────────────────────────────────────┘
```

---

## 🚀 Complete Step-by-Step Setup Guide

Follow this exact order to set up your entire network in **under 3 minutes**:

### 1️⃣ Step 1: Install Gateway VPS (Hub)
Run this command on your **Public Gateway VPS** (e.g. AWS EC2 `3.108.55.144`):
```bash
bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/install.sh)
```
- In the interactive menu, select **`[ 1 ] Install / Setup Gateway VPS (Hub)`**.
- Copy the **Gateway Public Key** displayed on the screen.

---

### 2️⃣ Step 2: Install Pterodactyl Node VPS (Spoke)
Run this command on your **Backend Node VPS** where Pterodactyl Wings is running:
```bash
bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/install.sh)
```
- In the interactive menu, select **`[ 2 ] Install / Setup Pterodactyl Node VPS (Spoke)`**.
- Enter your Gateway Public IP (`3.108.55.144`) and paste the Gateway Public Key from Step 1.

---

### 3️⃣ Step 3: Authorize Node Key on Gateway (Crucial)
1. On your **Node VPS**, copy your Node public key:
   ```bash
   cat /etc/wireguard/node_public.key
   ```
2. On your **Gateway VPS**, run:
   ```bash
   wg set wg0 peer <PASTE_NODE_PUBLIC_KEY> allowed-ips 10.200.0.2/32
   ```
3. Test tunnel link on Node:
   ```bash
   ping -c 2 10.200.0.1
   ```
   *(You will see `< 15ms` sub-millisecond response!)*

---

### 4️⃣ Step 4: Activate High-Speed Rust Engine (On Both Servers)
Run this single command on **both Gateway and Node VPS**:
```bash
wirenet daemon install
```
- Gateway runs `wirenet-gateway.service` (Control plane, Anti-DDoS).
- Node runs `wirenet-node.service` (Docker watcher & local stream bridge).

---

## ⚡ How Real Player IPs Work (Zero Plugins Required!)

WireNet uses **Pure Kernel Layer-3 Transparent Routing**:
- The Gateway preserves the player's true source IP (`104.28.x.x`).
- The Node's policy routing (`CONNMARK` + `Table 100`) routes replies symmetrically back through the Gateway.
- **ZERO PLUGINS ARE NEEDED.** Works natively on Vanilla, Paper, Purpur, Fabric, Forge, and Bedrock out-of-the-box!

---

## 🕹️ Unified `wirenet` Command Reference

You can run **`wirenet`** from any terminal directory:

| Command | Action |
|---|---|
| **`wirenet`** | Opens the **Interactive Control Center (TUI)** with arrow key navigation |
| **`wirenet tui`** | Launches the **Zero-Flicker Live Telemetry & Real IP Dashboard** |
| **`wirenet doctor`** | Runs the **6-point system health inspector & auto-repair** |
| **`wirenet status`** | Displays active tunnel peers, latency, and game ports |
| **`wirenet shield standard`** | Enables standard hardware SYN cookies & per-IP rate limiting |
| **`wirenet shield strict`** | Enables aggressive anti-bot raid mitigation mode |
| **`wirenet shield off`** | Disables shield (pass-through diagnostic mode) |
| **`wirenet daemon install`** | 🦀 Compiles and installs the Rust daemon into systemd |
| **`wirenet daemon status`** | Checks status of background Rust daemon service |
| **`wirenet update`** | Synchronizes all scripts & binaries from GitHub in 0 seconds |

---

## 🧹 Complete 100% Deep Uninstaller

If you ever need to restore your server to its original state, run:
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/scripts/uninstall.sh | sudo bash
```
*(Completely wipes all services, binaries, routing tables, and firewall rules).*

---

## 📜 License
Developed with ❤️ by **[UG88](https://github.com/UG88)** under the [MIT License](LICENSE).
