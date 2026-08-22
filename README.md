# WireNet — Kernel-Level Minecraft Ingress & Anti-DDoS Shield for Pterodactyl

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![GitHub Repository](https://img.shields.io/badge/GitHub-UG88%2Fwirenet-blue.svg)](https://github.com/UG88/wirenet)

**WireNet** is a production-grade, kernel-level networking and anti-DDoS shield designed specifically for Minecraft and game server hosting providers operating on [Pterodactyl](https://pterodactyl.io).

It replaces userspace reverse proxies (like FRP) with native Linux kernel transparent tunneling (WireGuard) and automated port bridging (`rinetd` / `iptables`), delivering:
- **100% Native Real Player IPs** — Zero plugins required on any server flavor (Vanilla, Fabric, Forge, NeoForge, Paper, Purpur, Spigot, Bedrock, Geyser) or via PROXY Protocol v2.
- **100% Hidden Backend Node IPs** — Protects Pterodactyl node hardware, storage, and databases from direct internet scans and attacks.
- **Built-in Kernel Anti-DDoS & Packet Filtering Shield** — Hardware SYN cookies, malformed packet dropper, bot join throttler, and Bedrock UDP reflection shields.
- **Multi-Node & Multi-IP Scaling** — Route multiple dedicated public IPs to different backend nodes so every customer VM gets default port `25565`.
- **Zero-Downtime Dynamic Reconfiguration** — Toggle or upgrade firewall modes without disconnecting active players.
- **Automatic Server Support** — All newly created servers in Pterodactyl on ports `25565-25600` and `30000-40050` work out of the box with zero manual configuration.
- **Interactive TUI Control Center** — Arrow-key navigable master manager for instant installation, doctor diagnostics, and 1-click repairs.

---

## 🎮 1-Command All-In-One Control Center

Launch the interactive **TUI Master Menu** on **any VPS** with full arrow-key navigation (Hyper-V / Proxmox style):

```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/wirenet.sh | sudo bash
```

```
================================================================================
       __      ___          _   _      _     __  __                             
       \ \    / (_)        | \ | |    | |   |  \/  |                            
        \ \  / / _ _ __ ___|  \| | ___| |_  | \  / | __ _ _ __   __ _  __ _ ___ 
         \ \/ / | | '__/ _ \ . ` |/ _ \ __| | |\/| |/ _` | '_ \ / _` |/ _` / __|
          \  /  | | | |  __/ |\  |  __/ |_  | |  | | (_| | | | | (_| | (_| \__ \
           \/   |_|_|  \___|_| \_|\___|\__| |_|  |_|\__,_|_| |_|\__,_|\__, |___/
                                                                        __/ |   
                                                                       |___/    
               Kernel-Level Ingress & Anti-DDoS Shield for Pterodactyl          
================================================================================
  Use UP/DOWN Arrow Keys to select, press ENTER (or press number 1-9):

  ▶ [ 1 ] Install / Setup Gateway VPS (Hub) 
    [ 2 ] Install / Setup Pterodactyl Node VPS (Spoke)
    [ 3 ] Live Status & Telemetry Dashboard
    [ 4 ] Run WireNet Doctor (Full Diagnostic & Auto-Fix)
    [ 5 ] Minecraft Anti-DDoS Shield Manager
    [ 6 ] Custom Port Forwarder (e.g. 25567 -> 25565)
    [ 7 ] Dedicated Multi-IP Mapping Tool
    [ 8 ] Advanced Troubleshooting & Auto-Repair Menu
    [ 9 ] Uninstall WireNet Completely
    [ 10] Exit WireNet Manager
```

---

## 1. Network Topology

```
[ Public Internet / Players (Player IP: 104.28.228.88) ]
                     │
                     │ Connects to Gateway Public IP: 3.108.55.144:25565
                     ▼
┌───────────────────────────────────────────────────────────────┐
│                 WireNet Gateway VPS (Hub)                     │
│  - Public IP: 3.108.55.144                                    │
│  - WireGuard Virtual IP: 10.200.0.1                           │
│  - Kernel Packet Filter & SYN Cookie Anti-DDoS Shield         │
│  - Automated Port Forwarding to Backend Nodes                 │
└───────────────────────────────┬───────────────────────────────┘
                                │
                                │ Encrypted WireGuard Kernel Pipe (ChaCha20-Poly1305)
                                ▼
┌───────────────────────────────────────────────────────────────┐
│              Private Pterodactyl Node VPS (Spoke)             │
│  - Backend Public IP is 100% HIDDEN                           │
│  - WireGuard Virtual IP: 10.200.0.2                           │
│  - Automated rinetd Docker Port Bridge (25565-25600, 30000+)  │
└───────────────────────────────┬───────────────────────────────┘
                                │
                                ▼
┌───────────────────────────────────────────────────────────────┐
│              Customer's Minecraft Server (Docker)             │
│  - Receives player connections seamlessly                     │
│  - Works for Vanilla, Forge, Fabric, Paper, Bedrock           │
│  - /ban-ip <player> safely bans ONLY that player!             │
└───────────────────────────────────────────────────────────────┘
```

---

## 2. Direct CLI Scripts Reference

### Setup Gateway VPS
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/setup-gateway.sh | sudo bash
```

### Setup Pterodactyl Node VPS (With Interactive Node Menu)
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/setup-node.sh | sudo bash
```

### Run WireNet Doctor (6-Point Health Inspector)
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/doctor.sh | sudo bash
```

### View Live Telemetry Dashboard
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/status.sh | sudo bash
```

### Universal 1-Click Auto-Troubleshooter
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/troubleshoot.sh | sudo bash
```

---

## 3. Multi-Node Scaling & New Server Creation

### When Creating a New Node (Node 2, Node 3...):
Run `setup-node.sh` on the new node and select the node number from the interactive menu:
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/setup-node.sh | sudo bash
```

Then authorize on Gateway VPS:
```bash
sudo wg set wg0 peer <NODE_PUBLIC_KEY> allowed-ips 10.200.0.3/32
sudo ip route add 10.200.0.3 dev wg0 2>/dev/null || true
```

### When Creating a New Server in Pterodactyl:
Any port assigned to the server in Pterodactyl within ranges **`25565-25600`** or **`30000-30050`** will automatically bridge and route through the Gateway with **zero manual configuration**!

---

## 4. Minecraft Anti-DDoS & Firewall Management

Manage the kernel packet filtering shield on your Gateway VPS with **zero player disconnections**:

```bash
# View live attack statistics and dropped packet counters
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/firewall.sh) status

# Turn on standard hosting protection
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/firewall.sh) enable

# Turn on STRICT anti-bot mode (during heavy bot raids)
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/firewall.sh) strict

# Temporarily disable shield
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/firewall.sh) disable
```

---

## 5. Script Summary Reference

| Script | Server Location | Purpose |
|---|---|---|
| [`wirenet.sh`](wirenet.sh) | **Any VPS** | Interactive Master TUI Menu with arrow-key navigation |
| [`doctor.sh`](doctor.sh) | **Any VPS** | 6-point system health inspector & automated repair |
| [`status.sh`](status.sh) | **Any VPS** | Live telemetry, peer table, and game ports dashboard |
| [`troubleshoot.sh`](troubleshoot.sh) | **Any VPS** | Universal 1-click self-healing diagnostic & repair tool |
| [`setup-gateway.sh`](setup-gateway.sh) | **Gateway VPS** | Deploys WireGuard Hub, port forwards, and Anti-DDoS Shield |
| [`setup-node.sh`](setup-node.sh) | **Node VPS** | Interactive Node setup with automated `rinetd` Docker bridge |
| [`forward-port.sh`](forward-port.sh) | **Gateway VPS** | 1-command custom port forwarder / translator |
| [`add-ip-mapping.sh`](add-ip-mapping.sh) | **Gateway VPS** | Maps dedicated Public IPs to specific backend Nodes |
| [`firewall.sh`](firewall.sh) | **Gateway VPS** | Controls the dynamic firewall (`status`, `enable`, `strict`, `disable`) |
| [`uninstall.sh`](uninstall.sh) | **Any VPS** | Completely wipes WireNet services and configuration |

---

## 6. Error Fix Guide

For complete diagnostic flowcharts, see **[`TROUBLESHOOTING.md`](TROUBLESHOOTING.md)**.

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
