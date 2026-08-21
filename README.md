# WireNet — Kernel-Level Minecraft Ingress & Anti-DDoS Shield for Pterodactyl

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**WireNet** is a high-performance, kernel-level networking and anti-DDoS shield designed specifically for game hosting providers and Minecraft server networks running on [Pterodactyl](https://pterodactyl.io).

It replaces userspace proxies (like FRP) with native Linux kernel transparent tunneling (WireGuard) and packet filtering (`iptables`/`conntrack`), delivering:
- **100% Native Real Player IPs** (Zero plugins required on any server: Vanilla, Fabric, Forge, Paper, Purpur, Bedrock, Geyser).
- **100% Hidden Backend Node IPs** (Protects Pterodactyl node hardware, storage, and databases from direct internet attacks).
- **Built-in Kernel Anti-DDoS & Packet Filtering Shield** (Mitigates TCP SYN floods, drops malformed scanning packets, throttles bot joins, and blocks Bedrock UDP reflection floods).
- **Zero-Downtime Dynamic Reconfiguration** (Toggle or update firewall modes without disconnecting active players).

---

## Network Architecture

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

## Fast Setup Guide

### Step 1: Set Up the Gateway VPS (Run on Gateway VPS)

```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/setup-gateway.sh | sudo bash
```

**What this does automatically:**
1. Installs official `wireguard`, `wireguard-tools`, and `iptables`.
2. Applies kernel network tuning and cryptographic SYN cookie protections.
3. Configures the `10.200.0.1` WireGuard hub.
4. Sets up DNAT port forwarding for ports `25565–25600` and `30000–40000`.
5. Deploys the Minecraft Anti-DDoS and Packet Filtering Shield.
6. **Prints the Gateway Public IP and Gateway Public Key.**

---

### Step 2: Set Up the Pterodactyl Node VPS (Run on Node VPS)

Replace `YOUR_GATEWAY_IP` and `YOUR_GATEWAY_PUBLIC_KEY` with the details from Step 1:

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

Paste the authorization command printed by Step 2 into your Gateway terminal:
```bash
sudo wg set wg0 peer <NODE_PUBLIC_KEY> allowed-ips 10.200.0.2/32
```

---

### Step 4: Verify Connectivity

On your **Pterodactyl Node VPS**, test the tunnel:
```bash
ping -c 3 10.200.0.1
```
*(Should return `<1ms` ping!)*

---

## Managing the Minecraft Firewall Shield

You can manage the anti-DDoS shield on your Gateway VPS at any time with **zero downtime**:

```bash
# Check live attack statistics and dropped packet counters
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/firewall.sh) status

# Turn on standard protection
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/firewall.sh) enable

# Turn on strict mode (during active massive bot raids)
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/firewall.sh) strict

# Temporarily disable shield (zero player disconnects)
sudo bash <(curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/firewall.sh) disable
```

---

## Uninstallation

To cleanly remove WireNet from any VPS:
```bash
curl -fsSL https://raw.githubusercontent.com/UG88/wirenet/main/uninstall.sh | sudo bash
```

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
