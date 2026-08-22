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

### 1️⃣ Step 1: Install WireNet on Gateway VPS (Hub)
Run this command on your **Public Gateway VPS** (e.g. AWS EC2 `3.108.55.144`):
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/install.sh | sudo bash
```
Then configure the gateway with one command:
```bash
wirenet setup gateway
```
- Copy the **Gateway Public Key** displayed on the screen.

---

### 2️⃣ Step 2: Install WireNet on Pterodactyl Node VPS (Spoke)
Run this command on your **Backend Node VPS** where Pterodactyl Wings is running:
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/install.sh | sudo bash
```
Then connect to the gateway with one command:
```bash
wirenet setup node --gateway 3.108.55.144 --gateway-key "<PASTE_GATEWAY_PUBLIC_KEY>"
```

---

### 3️⃣ Step 3: Authorize Node Key on Gateway (Crucial)
1. On your **Gateway VPS**, authorize the Node:
   ```bash
   wg set wg0 peer <PASTE_NODE_PUBLIC_KEY> allowed-ips 10.200.0.2/32
   ```
2. Test tunnel link on Node:
   ```bash
   wirenet doctor
   ```
   *(You will see all 6 health checks return `[✓]` instantly!)*

---

## 🕹️ Unified `wirenet` Command Reference

You can run **`wirenet`** from any terminal directory:

| Command | Action |
|---|---|
| **`wirenet setup gateway`** | 🌐 1-Click setup of WireGuard Hub and async Ingress Daemon |
| **`wirenet setup node`** | 🚀 1-Click setup of Spoke tunnel and Docker Auto-Discovery |
| **`wirenet doctor`** | 🩺 6-Point system diagnostic doctor & self-healing engine |
| **`wirenet status`** | 📊 Displays live tunnel status, latency, peers, and services |
| **`wirenet tui`** | 📈 Zero-Flicker Live Streaming Ratatui Dashboard & IP Sniffer |
| **`wirenet shield [mode]`** | 🛡️ Anti-DDoS profile switch (`standard`, `strict`, `off`) |
| **`wirenet check-update`** | 🔍 Check if a new version is available on GitHub |
| **`wirenet update`** | 🔄 1-Click self-updater from GitHub |
| **`wirenet uninstall`** | 🧹 100% deep cleaner and complete system uninstaller |

---

## 🧹 Complete 100% Deep Uninstaller

If you ever need to restore your server to its original state, simply run:
```bash
wirenet uninstall
```
*(Completely wipes all services, binaries, routing tables, and WireGuard configurations).*

---

## 📜 License
Developed with ❤️ by **[UG88](https://github.com/UG88)** under the [MIT License](LICENSE).
