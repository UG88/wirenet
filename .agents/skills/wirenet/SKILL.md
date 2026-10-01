---
name: wirenet
description: Comprehensive operational skill for WireNet Minecraft & Pterodactyl kernel-level ingress, WireGuard tunneling, Anti-DDoS management, real-time TUI telemetry, and auto-diagnostics.
---

# WireNet Operational Skill (Method 1: Pure Zero-Plugin Kernel Routing)

Use this skill whenever working on, debugging, configuring, deploying, or testing WireNet components across Gateway and Node VPS environments.

---

## 🛠️ CLI Quick Reference

| Task | Command |
|---|---|
| **Setup Gateway VPS (Hub)** | `wirenet setup gateway` |
| **Setup Node VPS (Spoke)** | `wirenet setup node --gateway <IP> --gateway-key <KEY>` |
| **Live Zero-Flicker Telemetry TUI** | `wirenet tui` |
| **6-Point System Doctor & Self-Healing** | `wirenet doctor` |
| **Apply Real IP Routing In-Place** | `wirenet apply` |
| **Authorize & Persist Node Peer** | `wirenet peer add <NODE_PUBKEY>` |
| **Active Tunnel Status & Latency** | `wirenet status` |
| **Anti-DDoS Shield (Standard/Strict/Off)**| `wirenet shield [standard\|strict\|off]` |
| **Check for Updates** | `wirenet check-update` |
| **1-Click Self-Updater** | `wirenet update` |
| **100% Deep Uninstaller** | `wirenet uninstall` |

---

## 🏗️ Method 1 Kernel Routing Invariants (pfSense Architecture)

1. **Gateway Ingress (DNAT)**:
   - Pure Layer-3 DNAT from public port `25565:25700` to `10.200.0.2`.
   - **NO MASQUERADE on `wg0`**: Preserves player's true public IPv4 address (`104.28.x.x`).
   - Default forwarding policy: `iptables -P FORWARD ACCEPT` with UFW sync.

2. **Node Return Path (pfSense `reply-to` Model)**:
   - `wg0.conf`: `Table = off` with `AllowedIPs = 0.0.0.0/0`.
   - Incoming `wg0` packets marked with `CONNMARK (0x1)`.
   - `ip rule add fwmark 0x1 table 100` with default route `via 10.200.0.1 dev wg0`.
   - `sysctl -w net.ipv4.conf.all.rp_filter=0` (disabled reverse path drop).

3. **Direct Container IP Routing (`DOCKER-USER`)**:
   - Inspects running container internal IP (`172.18.0.x`) and adds direct DNAT rules.
   - Bypasses `docker-proxy` userland socket rewriting so Minecraft receives authentic player IP.
   - Explicitly permits bridge traffic in `DOCKER-USER` chain (`-i wg0 -j ACCEPT`, `-o wg0 -j ACCEPT`).

4. **Backend Node IP Invisibility**:
   - Direct public access on `eth0` for game ports is **100% BLOCKED (DROPPED)**.
   - Unconditional `ACCEPT` for `lo`, `127.0.0.0/8`, and `wg0` before `DROP` rules.

---

## 🩺 10-Point Network Diagnostic Matrix

| Check | Inspection Item | Command / Test | Resolution |
|---|---|---|---|
| **1** | Target Listening Socket | `ss -tulpn` or `docker ps` | Verify game process is listening on `0.0.0.0:<port>` |
| **2** | Tunnel Latency & Connectivity | `ping -c 3 10.200.0.1` | Check WireGuard handshake and peer endpoints |
| **3** | Linux IP Forwarding | `sysctl net.ipv4.ip_forward` | Enable `net.ipv4.ip_forward=1` |
| **4** | Forwarding Chain Policy | `iptables -S FORWARD` | Set default policy to `ACCEPT` or permit `wg0` |
| **5** | UFW / OS Firewall Status | `ufw status` | Run `ufw allow <port>` or add bypass in `DOCKER-USER` |
| **6** | Cloud Security Groups | AWS EC2 / DO Firewall console | Open inbound TCP/UDP on public game & tunnel ports |
| **7** | Container Internal IP Mapping | `docker inspect <id>` | Direct DNAT to container IP (`172.18.0.x`) |
| **8** | Policy Return Route Table | `ip rule show` & `ip route show table 100` | Verify `fwmark 0x1 table 100` default gateway `10.200.0.1` |
| **9** | Reverse Path Filtering | `sysctl net.ipv4.conf.all.rp_filter` | Set `rp_filter=0` on `all`, `default`, and `wg0` |
| **10** | Live Packet Sniffing | `tcpdump -nn -i wg0 port <port>` | Inspect live packet flow entering and exiting interfaces |

---

## 🔍 1-Click System Doctor & Self-Healing:
```bash
wirenet doctor
```

*For detailed pfSense networking and NAT reference, see [`.ai/pfsense-knowledge.md`](file:///g:/FRP-Ports/.ai/pfsense-knowledge.md).*
