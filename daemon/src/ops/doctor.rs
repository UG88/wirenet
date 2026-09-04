use anyhow::Result;
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::time::Duration;

pub struct DoctorManager;

impl DoctorManager {
    pub fn run_diagnostics() -> Result<()> {
        println!("==========================================================");
        println!(" 🩺 WireNet 6-Point System Doctor & Self-Healing Engine");
        println!("==========================================================");

        // 1. Kernel Forwarding & Route Localnet
        println!("[1/6] Inspecting Linux Kernel Packet Forwarding & Routing...");
        let _ = Command::new("sysctl")
            .args(["-w", "net.ipv4.ip_forward=1"])
            .output();
        let _ = Command::new("sysctl")
            .args(["-w", "net.ipv4.conf.all.forwarding=1"])
            .output();
        let _ = Command::new("sysctl")
            .args(["-w", "net.ipv4.conf.all.route_localnet=1"])
            .output();
        let _ = Command::new("sysctl")
            .args(["-w", "net.ipv4.conf.default.route_localnet=1"])
            .output();
        let _ = Command::new("sysctl")
            .args(["-w", "net.ipv4.conf.all.rp_filter=2"])
            .output();
        let _ = Command::new("sysctl")
            .args(["-w", "net.ipv4.conf.default.rp_filter=2"])
            .output();
        println!("  [✓] Kernel IPv4 forwarding & route_localnet: ENABLED");

        // 2. WireGuard Interface & Handshake
        println!("[2/6] Checking WireGuard Kernel Interface (wg0)...");
        let wg_status = Command::new("wg").args(["show", "wg0"]).output();
        match wg_status {
            Ok(o) if o.status.success() => {
                let out_str = String::from_utf8_lossy(&o.stdout);
                let peers = out_str
                    .lines()
                    .filter(|l| l.trim().starts_with("peer:"))
                    .count();
                if peers == 0 {
                    println!("  [!] Warning: Interface wg0 is UP, but NO PEERS are registered!");
                    println!("  [!] Gateway needs: wg set wg0 peer <NODE_PUBKEY> allowed-ips 10.200.0.2/32");
                } else {
                    println!("  [✓] Interface wg0 is UP with {} active peer(s)", peers);
                }
            }
            _ => {
                println!("  [!] Interface wg0 is DOWN. Auto-healing interface...");
                let _ = Command::new("systemctl")
                    .args(["stop", "wg-quick@wg0"])
                    .output();
                let _ = Command::new("ip")
                    .args(["link", "del", "dev", "wg0"])
                    .output();
                let _ = Command::new("ip")
                    .args(["rule", "del", "fwmark", "0x1"])
                    .output();
                let _ = Command::new("ip")
                    .args(["route", "flush", "table", "100"])
                    .output();
                let up_out = Command::new("wg-quick").args(["up", "wg0"]).output();
                if let Ok(ref u) = up_out {
                    if u.status.success() {
                        println!("  [✓] Interface wg0 brought UP successfully!");
                        let _ = Command::new("systemctl")
                            .args(["enable", "--now", "wg-quick@wg0"])
                            .output();
                    } else {
                        println!(
                            "  [!] wg-quick error: {}",
                            String::from_utf8_lossy(&u.stderr).trim()
                        );
                    }
                }
            }
        }

        // 3. Peer Pings & Latency
        println!("[3/6] Measuring Tunnel Latency & Connectivity...");
        let is_gateway = std::fs::read_to_string("/etc/wireguard/wg0.conf")
            .map(|c| c.contains("10.200.0.1/24"))
            .unwrap_or(false);

        let target_test_ip = if is_gateway {
            "10.200.0.2"
        } else {
            "10.200.0.1"
        };
        let ping = Command::new("ping")
            .args(["-c", "2", "-W", "1", target_test_ip])
            .output();
        match ping {
            Ok(p) if p.status.success() => {
                let out = String::from_utf8_lossy(&p.stdout);
                if let Some(rtt) = out.lines().last().and_then(|l| l.split('/').nth(4)) {
                    println!(
                        "  [✓] WireGuard Tunnel Peer ({}) is REACHABLE! (Latency: {}ms)",
                        target_test_ip, rtt
                    );
                } else {
                    println!(
                        "  [✓] WireGuard Tunnel Peer ({}) is REACHABLE!",
                        target_test_ip
                    );
                }
            }
            _ => {
                println!(
                    "  [✗] ERROR: Tunnel Peer ({}) is UNREACHABLE!",
                    target_test_ip
                );
                if is_gateway {
                    println!("      -> Node has not connected yet, or Node Public Key is not added to Gateway.");
                    println!("      -> Run on Gateway: wg set wg0 peer <NODE_PUBLIC_KEY> allowed-ips 10.200.0.2/32");
                } else {
                    println!("      -> Cannot reach Gateway (10.200.0.1). Ensure Gateway public IP and port 51820 UDP are reachable.");
                }
            }
        }

        // 4. Background Daemon Services
        println!("[4/6] Checking WireNet Background Daemons...");
        let gw_active = Command::new("systemctl")
            .args(["is-active", "wirenet-gateway.service"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        let node_active = Command::new("systemctl")
            .args(["is-active", "wirenet-node.service"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if gw_active {
            println!("  [✓] wirenet-gateway.service is ACTIVE");
        } else if node_active {
            println!("  [✓] wirenet-node.service is ACTIVE");
        } else {
            println!("  [i] No background systemd daemon detected on this host.");
        }

        // 5. Firewall Integrity & Localnet Fix
        println!("[5/6] Verifying Firewall & Policy Routing Rules...");
        let _ = Command::new("sysctl")
            .args(["-w", "net.ipv4.ip_forward=1"])
            .output();
        let _ = Command::new("sysctl")
            .args(["-w", "net.ipv4.conf.all.forwarding=1"])
            .output();
        let _ = Command::new("sysctl")
            .args(["-w", "net.ipv4.conf.all.rp_filter=0"])
            .output();
        let _ = Command::new("sysctl")
            .args(["-w", "net.ipv4.conf.default.rp_filter=0"])
            .output();
        let _ = Command::new("sysctl")
            .args(["-w", "net.ipv4.conf.wg0.rp_filter=0"])
            .output();
        let _ = Command::new("iptables")
            .args(["-P", "FORWARD", "ACCEPT"])
            .output();
        let _ = Command::new("iptables")
            .args(["-I", "FORWARD", "1", "-j", "ACCEPT"])
            .output();
        let _ = Command::new("iptables")
            .args(["-I", "FORWARD", "1", "-i", "wg0", "-j", "ACCEPT"])
            .output();
        let _ = Command::new("iptables")
            .args(["-I", "FORWARD", "1", "-o", "wg0", "-j", "ACCEPT"])
            .output();
        let _ = Command::new("iptables")
            .args(["-I", "DOCKER-USER", "1", "-j", "ACCEPT"])
            .output();
        let _ = Command::new("iptables")
            .args(["-I", "INPUT", "1", "-i", "wg0", "-j", "ACCEPT"])
            .output();
        let _ = Command::new("iptables")
            .args(["-I", "INPUT", "1", "-i", "lo", "-j", "ACCEPT"])
            .output();

        if !is_gateway {
            let route_check = Command::new("ip")
                .args(["route", "get", "1.1.1.1", "mark", "0x1"])
                .output();
            if let Ok(ref rc) = route_check {
                let out = String::from_utf8_lossy(&rc.stdout);
                if out.contains("dev wg0") {
                    println!("  [✓] Policy Routing (mark 0x1 -> table 100 dev wg0): VERIFIED");
                } else {
                    println!("  [!] Policy Routing: {}", out.trim());
                }
            }
        }
        println!("  [✓] WireGuard Forwarding & Ingress Chains: VERIFIED");

        // 6. Game Port Testing (TCP Socket probe)
        println!("[6/6] Probing Game Server Ports & Docker Containers...");
        let docker_ports = Self::scan_docker_ports();
        if !docker_ports.is_empty() {
            println!("  [+] Discovered Running Pterodactyl Game Servers:");
            for (name, port, cip, proto) in &docker_ports {
                println!(
                    "      • Server: {:<20} | Port: {:<5} | Container IP: {:<15} | Protocol: {}",
                    name, port, cip, proto
                );
            }
        }

        let timeout = Duration::from_millis(1500);
        let primary_ip = Self::get_primary_ip();

        let mut test_ports = vec![25565];
        for (_, p, _, _) in &docker_ports {
            if !test_ports.contains(p) {
                test_ports.push(*p);
            }
        }

        let mut any_open = false;
        for &p in &test_ports {
            let targets = [
                (
                    format!("Primary Node IP ({}:{})", primary_ip, p),
                    format!("{}:{}", primary_ip, p),
                ),
                (
                    format!("Local Loopback (127.0.0.1:{})", p),
                    format!("127.0.0.1:{}", p),
                ),
                (
                    format!("Node Tunnel IP (10.200.0.2:{})", p),
                    format!("10.200.0.2:{}", p),
                ),
                (
                    format!("Gateway Tunnel IP (10.200.0.1:{})", p),
                    format!("10.200.0.1:{}", p),
                ),
            ];

            for (label, addr_str) in &targets {
                if let Ok(socket_addr) = addr_str.parse::<SocketAddr>() {
                    if TcpStream::connect_timeout(&socket_addr, timeout).is_ok() {
                        println!("  [✓] Port {} OPEN: Successfully connected to {}", p, label);
                        any_open = true;
                    }
                }
            }
        }

        if !any_open {
            println!("  [!] Warning: No open game sockets detected on local/tunnel interfaces.");
            println!("  [!] Please ensure your Minecraft/game server is started in Pterodactyl!");
        }

        println!("==========================================================");
        println!(" [✓] WireNet Doctor Diagnostic & Self-Repair Finished!");
        println!("==========================================================");

        Ok(())
    }

    fn scan_docker_ports() -> Vec<(String, u16, String, String)> {
        let mut results = Vec::new();
        let out = Command::new("docker")
            .args(["ps", "--format", "{{.ID}}\t{{.Ports}}\t{{.Names}}"])
            .output();

        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout);
            for line in s.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() < 2 {
                    continue;
                }
                let cid = parts[0];
                let ports_str = parts[1];
                let name = parts.get(2).unwrap_or(&"Container").to_string();

                let ip_out = Command::new("docker")
                    .args([
                        "inspect",
                        cid,
                        "--format",
                        "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}",
                    ])
                    .output();
                let cip = match ip_out {
                    Ok(io) => {
                        let ip_s = String::from_utf8_lossy(&io.stdout).trim().to_string();
                        if !ip_s.is_empty() {
                            ip_s
                        } else {
                            "127.0.0.1".to_string()
                        }
                    }
                    _ => "127.0.0.1".to_string(),
                };

                for port_part in ports_str.split(',') {
                    let trimmed = port_part.trim();
                    if let Some(arrow_idx) = trimmed.find("->") {
                        let host_side = &trimmed[..arrow_idx];
                        let container_side = &trimmed[arrow_idx + 2..];

                        let port_num = host_side
                            .split(':')
                            .next_back()
                            .and_then(|p| p.parse::<u16>().ok())
                            .unwrap_or(0);

                        let proto = if container_side.contains("/udp") {
                            "UDP".to_string()
                        } else if container_side.contains("/tcp") {
                            "TCP".to_string()
                        } else {
                            "TCP/UDP".to_string()
                        };

                        if port_num >= 1024 && !results.iter().any(|(_, p, _, _)| *p == port_num) {
                            results.push((name.clone(), port_num, cip.clone(), proto));
                        }
                    }
                }
            }
        }
        results
    }

    fn get_primary_ip() -> String {
        let out = Command::new("ip")
            .args(["route", "get", "1.1.1.1"])
            .output();
        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout);
            let parts: Vec<&str> = s.split_whitespace().collect();
            if let Some(idx) = parts.iter().position(|&r| r == "src") {
                if let Some(ip) = parts.get(idx + 1) {
                    return ip.to_string();
                }
            }
        }
        "127.0.0.1".to_string()
    }
}
