# WireNet — Kernel-Level Minecraft Ingress & Anti-DDoS Shield for Pterodactyl

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![GitHub Repository](https://img.shields.io/badge/GitHub-UG88%2Fwirenet-blue.svg)](https://github.com/UG88/wirenet)
[![Rust Engine](https://img.shields.io/badge/Engine-Async%20Tokio%20Rust-orange.svg)](daemon/)

**WireNet** is an enterprise, high-performance tunneling and Anti-DDoS protection system designed specifically for Minecraft and game hosting providers operating on [Pterodactyl](https://pterodactyl.io).

It replaces slow userspace reverse proxies (like FRP) with native Linux kernel WireGuard tunneling and a unified, memory-safe **Rust Daemon (`wirenet-daemon`)**, delivering:
- **100% Real Player IPs** — True client home IPs passed to Paper, Purpur, Velocity, BungeeCord, Fabric, and Geyser via PROXY Protocol v2.
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
│  - PROXY Protocol v2 Encoder (Injects Real Client IP)         │
│  - Asynchronous Tokio TCP/UDP Stream Ingress                  │
└───────────────────────────────┬───────────────────────────────┘
                                │
                                │ Encrypted WireGuard Kernel Fastpath (0.8ms RTT)
                                ▼
┌───────────────────────────────────────────────────────────────┐
│              Private Pterodactyl Node VPS (Spoke)             │
│  - Backend Public IP is 100% HIDDEN & INVISIBLE               │
│  - Private Virtual IP: 10.200.0.2                             │
│  - Docker /docker.sock Auto-Discovery (0ms port sync)         │
│  - Local Direct Container Bridge (25565-25700, 30000+)        │
└───────────────────────────────┬───────────────────────────────┘
                                │
                                ▼
┌───────────────────────────────────────────────────────────────┐
│              Customer's Minecraft Server (Docker)             │
│  - Receives genuine player IP: 104.28.228.88                  │
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
- Gateway runs `wirenet-gateway.service` (Ingress, Anti-DDoS, PROXY v2).
- Node runs `wirenet-node.service` (Docker watcher & local stream bridge).

---

## ⚡ How to Get 100% Real Player IPs in Minecraft

Because WireNet uses the universal enterprise **PROXY Protocol v2** standard, you can receive the exact player home IP on any server flavor:

### A. For Velocity Proxy (`velocity.toml`)
Open `velocity.toml` and set:
```toml
# Enable PROXY Protocol v2 support
haproxy-protocol = true
```

### B. For BungeeCord (`config.yml`)
Open `config.yml` and set:
```yaml
proxy_protocol: true
```

### C. For Paper / Purpur / Spigot / Folia Server (Backend)
If running a standalone Paper/Purpur server (without Velocity):
1. Download the free lightweight plugin **[HAProxyDetectorPaper on Modrinth](https://modrinth.com/plugin/haproxydetectorpaper)**.
2. Drop `HAProxyDetectorPaper.jar` into your Minecraft server's `plugins/` folder.
3. Restart your server.
4. ✨ **Done!** Every player's genuine home IP (`104.28.x.x`) will now appear in your console, `/seen`, and ban managers!

### D. For Geyser / Bedrock (`config.yml`)
Open Geyser's `config.yml` and set:
```yaml
# Enable PROXY protocol for Bedrock connections
proxy-protocol: true
```

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
