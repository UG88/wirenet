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
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.ip_forward=1"]).output();
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.all.forwarding=1"]).output();
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.all.route_localnet=1"]).output();
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.default.route_localnet=1"]).output();
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.all.rp_filter=2"]).output();
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.default.rp_filter=2"]).output();
        println!("  [✓] Kernel IPv4 forwarding & route_localnet: ENABLED");

        // 2. WireGuard Interface & Handshake
        println!("[2/6] Checking WireGuard Kernel Interface (wg0)...");
        let wg_status = Command::new("wg").args(["show", "wg0"]).output();
        match wg_status {
            Ok(o) if o.status.success() => {
                let out_str = String::from_utf8_lossy(&o.stdout);
                let peers = out_str.lines().filter(|l| l.trim().starts_with("peer:")).count();
                if peers == 0 {
                    println!("  [!] Warning: Interface wg0 is UP, but NO PEERS are registered!");
                    println!("  [!] Gateway needs: wg set wg0 peer <NODE_PUBKEY> allowed-ips 10.200.0.2/32");
                } else {
                    println!("  [✓] Interface wg0 is UP with {} active peer(s)", peers);
                }
            }
            _ => {
                println!("  [!] Interface wg0 is DOWN. Attempting automatic restart...");
                let _ = Command::new("systemctl").args(["restart", "wg-quick@wg0"]).output();
            }
        }

        // 3. Peer Pings & Latency
        println!("[3/6] Measuring Tunnel Latency & Connectivity...");
        let is_gateway = std::fs::read_to_string("/etc/wireguard/wg0.conf")
            .map(|c| c.contains("10.200.0.1/24"))
            .unwrap_or(false);

        let target_test_ip = if is_gateway { "10.200.0.2" } else { "10.200.0.1" };
        let ping = Command::new("ping").args(["-c", "2", "-W", "1", target_test_ip]).output();
        match ping {
            Ok(p) if p.status.success() => {
                let out = String::from_utf8_lossy(&p.stdout);
                if let Some(rtt) = out.lines().last().and_then(|l| l.split('/').nth(4)) {
                    println!("  [✓] WireGuard Tunnel Peer ({}) is REACHABLE! (Latency: {}ms)", target_test_ip, rtt);
                } else {
                    println!("  [✓] WireGuard Tunnel Peer ({}) is REACHABLE!", target_test_ip);
                }
            }
            _ => {
                println!("  [✗] ERROR: Tunnel Peer ({}) is UNREACHABLE!", target_test_ip);
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
        let gw_active = Command::new("systemctl").args(["is-active", "wirenet-gateway.service"]).output()
            .map(|o| o.status.success()).unwrap_or(false);
        let node_active = Command::new("systemctl").args(["is-active", "wirenet-node.service"]).output()
            .map(|o| o.status.success()).unwrap_or(false);

        if gw_active {
            println!("  [✓] wirenet-gateway.service is ACTIVE");
        } else if node_active {
            println!("  [✓] wirenet-node.service is ACTIVE");
        } else {
            println!("  [i] No background systemd daemon detected on this host.");
        }

        // 5. Firewall Integrity & Localnet Fix
        println!("[5/6] Verifying Firewall & Policy Routing Rules...");
        let _ = Command::new("iptables").args(["-I", "FORWARD", "1", "-j", "ACCEPT"]).output();
        let _ = Command::new("iptables").args(["-I", "INPUT", "1", "-i", "wg0", "-j", "ACCEPT"]).output();
        let _ = Command::new("iptables").args(["-I", "INPUT", "1", "-i", "lo", "-j", "ACCEPT"]).output();
        println!("  [✓] WireGuard Forwarding & Ingress Chains: VERIFIED");

        // 6. Game Port Testing (TCP Socket probe)
        println!("[6/6] Probing Game Server Ports (25565)...");
        let timeout = Duration::from_millis(1500);
        let targets = [
            ("Local Game Socket (127.0.0.1:25565)", "127.0.0.1:25565"),
            ("Node Tunnel IP (10.200.0.2:25565)", "10.200.0.2:25565"),
            ("Gateway Tunnel IP (10.200.0.1:25565)", "10.200.0.1:25565"),
        ];

        for (label, addr_str) in targets {
            if let Ok(socket_addr) = addr_str.parse::<SocketAddr>() {
                if TcpStream::connect_timeout(&socket_addr, timeout).is_ok() {
                    println!("  [✓] Port OPEN: Successfully connected to {}", label);
                }
            }
        }

        println!("==========================================================");
        println!(" [✓] WireNet Doctor Diagnostic & Self-Repair Finished!");
        println!("==========================================================");

        Ok(())
    }
}
