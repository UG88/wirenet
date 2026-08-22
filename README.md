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
- **1-Click Self-Healing Troubleshooter** — Automated diagnostic and repair tool fixes broken routes, missing keys, and Docker bridge NATs in 1 second.

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

## 2. Fast 2-Step Setup

### Step 1: Set Up Gateway VPS (Run on Gateway VPS)

On your **Public Gateway VPS** (`root@ip-172-31-15-89`):
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/setup-gateway.sh | sudo bash
```

---

### Step 2: Set Up Pterodactyl Node VPS (Run on Node VPS)

On your **Pterodactyl Node VPS** (`root@arsoftware`):
```bash
GW_ENDPOINT="YOUR_GATEWAY_PUBLIC_IP" GW_PUBLIC_KEY="YOUR_GATEWAY_PUBLIC_KEY" curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/setup-node.sh | sudo bash
```

---

## 3. 🛠️ 1-Click Auto-Troubleshooter & Health Check

If you ever experience connection timeouts or port binding issues, run this **1-command universal troubleshooter** on **either VPS**:

```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/troubleshoot.sh | sudo bash
```

*(This automatically detects whether it is running on the Gateway or Node, runs 5 health checks, cleans conflicting NAT rules, refreshes port bridges, and verifies end-to-end connectivity)*.

See [`TROUBLESHOOTING.md`](TROUBLESHOOTING.md) for full error flowcharts and manual diagnostics.

---

## 4. Multi-Node Scaling & New Server Creation

### When Creating a New Node (Node 2, Node 3...):
On the new Node VPS, assign a unique virtual IP (`10.200.0.3`):
```bash
NODE_IP="10.200.0.3" GW_ENDPOINT="GATEWAY_IP" GW_PUBLIC_KEY="GATEWAY_KEY" curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/setup-node.sh | sudo bash
```

Then authorize Node 2 on your Gateway VPS:
```bash
sudo wg set wg0 peer <NODE2_PUBLIC_KEY> allowed-ips 10.200.0.3/32
sudo ip route add 10.200.0.3 dev wg0 2>/dev/null || true
```

### When Creating a New Server in Pterodactyl:
Any port assigned to the server in Pterodactyl within ranges **`25565-25600`** or **`30000-30050`** will automatically bridge and route through the Gateway with **zero manual configuration**!

---

## 5. Custom Port Translation (e.g. Gateway 25567 ──► Node 25565)

To map an arbitrary Gateway port to a specific Node port:
```bash
# On Gateway VPS:
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/forward-port.sh | sudo bash -s -- 25567 10.200.0.3 25565
```

---

## 6. Minecraft Anti-DDoS & Firewall Management

Manage the kernel packet filtering shield on your Gateway VPS at any time with **zero player disconnections**:

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

## 7. Script Summary Reference

| Script | Server Location | Purpose |
|---|---|---|
| [`setup-gateway.sh`](setup-gateway.sh) | **Gateway VPS** | Deploys WireGuard Hub, port forwards, and Anti-DDoS Shield |
| [`setup-node.sh`](setup-node.sh) | **Node VPS** | Connects Pterodactyl Node with automated `rinetd` Docker bridge |
| [`troubleshoot.sh`](troubleshoot.sh) | **Any VPS** | Universal 1-click self-healing diagnostic & repair tool |
| [`troubleshoot-gateway.sh`](troubleshoot-gateway.sh) | **Gateway VPS** | Gateway health diagnostic and routing repair |
| [`troubleshoot-node.sh`](troubleshoot-node.sh) | **Node VPS** | Node health diagnostic and port bridge repair |
| [`forward-port.sh`](forward-port.sh) | **Gateway VPS** | 1-command custom port forwarder / translator |
| [`add-ip-mapping.sh`](add-ip-mapping.sh) | **Gateway VPS** | Maps dedicated Public IPs to specific backend Nodes |
| [`firewall.sh`](firewall.sh) | **Gateway VPS** | Controls the dynamic firewall (`status`, `enable`, `strict`, `disable`) |
| [`uninstall.sh`](uninstall.sh) | **Any VPS** | Completely wipes WireNet services and configuration |

---

## 8. Clean Uninstallation

To completely stop and remove WireNet from any VPS:
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/uninstall.sh | sudo bash
```

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
