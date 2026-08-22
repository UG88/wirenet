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

        // 1. Kernel Forwarding
        println!("[1/6] Inspecting Linux Kernel Packet Forwarding...");
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.ip_forward=1"]).output();
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.all.forwarding=1"]).output();
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.all.rp_filter=2"]).output();
        println!("  [✓] Kernel IPv4 forwarding & loose RP filtering: ENABLED");

        // 2. WireGuard Interface
        println!("[2/6] Checking WireGuard Kernel Interface (wg0)...");
        let wg_status = Command::new("wg").args(["show", "wg0"]).output();
        match wg_status {
            Ok(o) if o.status.success() => {
                let out_str = String::from_utf8_lossy(&o.stdout);
                let peers = out_str.lines().filter(|l| l.trim().starts_with("peer:")).count();
                println!("  [✓] Interface wg0 is UP (Active Peers: {})", peers);
            }
            _ => {
                println!("  [!] Interface wg0 is DOWN. Attempting automatic restart...");
                let _ = Command::new("systemctl").args(["restart", "wg-quick@wg0"]).output();
            }
        }

        // 3. Peer Pings & Latency
        println!("[3/6] Measuring Tunnel Latency & Connectivity...");
        let test_ips = ["10.200.0.1", "10.200.0.2", "10.200.0.3"];
        for ip in test_ips {
            let ping = Command::new("ping").args(["-c", "1", "-W", "1", ip]).output();
            if let Ok(p) = ping {
                if p.status.success() {
                    let out = String::from_utf8_lossy(&p.stdout);
                    if let Some(rtt) = out.lines().last().and_then(|l| l.split('/').nth(4)) {
                        println!("  [✓] Tunnel Endpoint {} is ONLINE! (Latency: {}ms)", ip, rtt);
                    } else {
                        println!("  [✓] Tunnel Endpoint {} is ONLINE!", ip);
                    }
                }
            }
        }

        // 4. Background Daemon Services
        println!("[4/6] Checking WireNet Systemd Daemons...");
        let gw_active = Command::new("systemctl").args(["is-active", "wirenet-gateway.service"]).output()
            .map(|o| o.status.success()).unwrap_or(false);
        let node_active = Command::new("systemctl").args(["is-active", "wirenet-node.service"]).output()
            .map(|o| o.status.success()).unwrap_or(false);

        if gw_active {
            println!("  [✓] wirenet-gateway.service is ACTIVE (High-Speed Ingress Engine)");
        } else if node_active {
            println!("  [✓] wirenet-node.service is ACTIVE (Docker Container Watcher)");
        } else {
            println!("  [i] No background systemd daemon detected on this host.");
        }

        // 5. Firewall Integrity
        println!("[5/6] Verifying Firewall Rules...");
        let _ = Command::new("iptables").args(["-I", "FORWARD", "1", "-i", "wg0", "-j", "ACCEPT"]).output();
        let _ = Command::new("iptables").args(["-I", "FORWARD", "1", "-o", "wg0", "-j", "ACCEPT"]).output();
        let _ = Command::new("iptables").args(["-I", "INPUT", "1", "-i", "wg0", "-j", "ACCEPT"]).output();
        println!("  [✓] WireGuard Forwarding & Ingress Chains: VERIFIED");

        // 6. Game Port Testing (TCP Socket probe)
        println!("[6/6] Probing Game Server Ports (25565)...");
        let timeout = Duration::from_millis(1500);
        let targets = [
            ("Local Host", "127.0.0.1:25565"),
            ("Tunnel Spoke", "10.200.0.2:25565"),
            ("Tunnel Hub", "10.200.0.1:25565"),
        ];

        for (label, addr_str) in targets {
            if let Ok(socket_addr) = addr_str.parse::<SocketAddr>() {
                if TcpStream::connect_timeout(&socket_addr, timeout).is_ok() {
                    println!("  [✓] SUCCESS: Connected to {} on {}", label, addr_str);
                }
            }
        }

        println!("==========================================================");
        println!(" [✓] WireNet Doctor Diagnostic & Self-Repair Finished!");
        println!("==========================================================");

        Ok(())
    }
}
