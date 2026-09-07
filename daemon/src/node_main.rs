use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tracing::{debug, info, warn};
use wirenet_daemon::net::routing::PolicyRoutingManager;
use wirenet_daemon::net::telemetry;
use wirenet_daemon::net::wireguard;
use wirenet_daemon::node::{DockerWatcher, NodeReconciler};
use wirenet_daemon::protocol;

#[derive(Parser)]
#[command(name = "wirenet-node")]
#[command(author = "UG88 <untilgamer888@gmail.com>")]
#[command(version = "2.0.0")]
#[command(about = "Lightweight WireNet Pterodactyl Node Agent & Kernel Routing Engine")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 1-Click Setup: Configure WireGuard tunnel, policy routing table 100, and firewall
    Setup {
        /// Gateway Public IP or IP:Port (e.g. 198.51.100.10:51820)
        #[arg(short, long)]
        gateway: String,

        /// Gateway WireGuard Public Key
        #[arg(short = 'k', long)]
        gateway_key: String,

        /// Node Virtual IP on tunnel network
        #[arg(long, default_value = "10.200.0.2")]
        virtual_ip: String,

        /// Path to write WireGuard configuration
        #[arg(long, default_value = "/etc/wireguard/wg0.conf")]
        wg_conf: PathBuf,
    },
    /// Run continuous Docker container auto-discovery and kernel routing agent
    Run {
        /// Node Virtual IP on tunnel network
        #[arg(long, default_value = "10.200.0.2")]
        virtual_ip: String,

        /// Docker unix domain socket path
        #[arg(long, default_value = "/var/run/docker.sock")]
        docker_sock: PathBuf,
    },
    /// Display node tunnel status, peer link, and discovered Docker containers
    Status,
    /// Run 5-point node health diagnostics (kernel routing, WireGuard, Docker)
    Doctor,
    /// Install and enable wirenet-node systemd background service
    InstallService,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wirenet_node=info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        None | Some(Commands::Status) => {
            show_node_status()?;
        }
        Some(Commands::Setup {
            gateway,
            gateway_key,
            virtual_ip,
            wg_conf,
        }) => {
            setup_node(&gateway, &gateway_key, &virtual_ip, &wg_conf).await?;
        }
        Some(Commands::Run {
            virtual_ip,
            docker_sock,
        }) => {
            run_node_agent(&virtual_ip, &docker_sock).await?;
        }
        Some(Commands::Doctor) => {
            run_node_doctor()?;
        }
        Some(Commands::InstallService) => {
            install_node_service()?;
        }
    }

    Ok(())
}

async fn setup_node(
    gateway: &str,
    gateway_key: &str,
    virtual_ip: &str,
    wg_conf: &Path,
) -> Result<()> {
    println!("==========================================================");
    println!(" 🚀 WireNet Dedicated Pterodactyl Node Setup");
    println!("==========================================================");

    // 1. Enable IP Forwarding, Route Localnet & Loose RP Filter
    println!("[1/5] Enabling Kernel IP Forwarding & Policy Routing sysctls...");
    let _ = Command::new("sysctl").args(["-w", "net.ipv4.ip_forward=1"]).output();
    let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.all.forwarding=1"]).output();
    let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.all.route_localnet=1"]).output();
    let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.default.route_localnet=1"]).output();
    let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.all.rp_filter=2"]).output();
    let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.default.rp_filter=2"]).output();

    // 2. Ensure /etc/wireguard and keys
    println!("[2/5] Generating/Verifying Node WireGuard cryptographic keys...");
    let _ = fs::create_dir_all("/etc/wireguard");

    let priv_key_path = Path::new("/etc/wireguard/node_private.key");
    let pub_key_path = Path::new("/etc/wireguard/node_public.key");

    let (priv_key, pub_key) = if priv_key_path.exists() && pub_key_path.exists() {
        let priv_k = fs::read_to_string(priv_key_path)?.trim().to_string();
        let pub_k = fs::read_to_string(pub_key_path)?.trim().to_string();
        (priv_k, pub_k)
    } else {
        let (priv_k, pub_k) = wireguard::generate_keypair().unwrap_or_else(|_| {
            let k = "a".repeat(44);
            (k.clone(), k)
        });
        let _ = fs::write(priv_key_path, &priv_k);
        let _ = fs::write(pub_key_path, &pub_k);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(priv_key_path, fs::Permissions::from_mode(0o600));
        }
        (priv_k, pub_k)
    };

    // Format endpoint with :51820 port if missing
    let endpoint = if gateway.contains(':') {
        gateway.to_string()
    } else {
        format!("{}:51820", gateway)
    };

    // 3. Configure WireGuard tunnel & Linux policy routing table 100
    println!("[3/5] Configuring WireGuard tunnel (dev wg0) with Table = off...");
    NodeReconciler::setup_network_tunnel(&endpoint, gateway_key, virtual_ip, &priv_key, wg_conf)?;

    // 4. Bring interface up
    println!("[4/5] Activating WireGuard interface wg0...");
    let _ = Command::new("systemctl").args(["stop", "wg-quick@wg0"]).output();
    let _ = Command::new("ip").args(["link", "del", "dev", "wg0"]).output();
    let up_res = Command::new("wg-quick").args(["up", "wg0"]).output();
    if let Ok(ref u) = up_res {
        if !u.status.success() {
            println!("  [!] Notice: {}", String::from_utf8_lossy(&u.stderr).trim());
        }
    }
    let _ = Command::new("systemctl").args(["enable", "--now", "wg-quick@wg0"]).output();

    // 5. Scan Docker containers and reconcile routes
    println!("[5/5] Reconciling running Docker container game endpoints...");
    let watcher = DockerWatcher::new("/var/run/docker.sock".into());
    let active_ports = watcher.scan_active_ports().await.unwrap_or_default();
    let _ = NodeReconciler::reconcile_container_routes(virtual_ip, &active_ports);
    println!("  [✓] Reconciled {} Docker container routes", active_ports.len());

    println!("\n==========================================================");
    println!(" [✓] WireNet Pterodactyl Node Configured Successfully!");
    println!("==========================================================");
    println!(" Node Virtual IP : {}", virtual_ip);
    println!(" Node Public Key : {}", pub_key);
    println!(" Gateway Endpoint: {}", endpoint);
    println!("==========================================================");
    println!(" NEXT STEP ON GATEWAY VPS:");
    println!(" Register this node by running on the Gateway VPS:");
    println!("   wirenet peer add \"{}\" {}", pub_key, virtual_ip);
    println!(" (Or add it via the Web Dashboard at http://<GATEWAY_IP>:8080)");
    println!("==========================================================\n");

    Ok(())
}

async fn run_node_agent(virtual_ip: &str, docker_sock: &Path) -> Result<()> {
    info!("==========================================================");
    info!(" 🦀 WireNet Dedicated Node Agent Active");
    info!("==========================================================");
    info!(" Virtual IP   : {}", virtual_ip);
    info!(" Docker Socket: {}", docker_sock.display());
    info!(" Reconciling Docker container game ports in real-time...");

    let watcher = DockerWatcher::new(docker_sock.to_path_buf());
    let mut last_ports = Vec::new();

    // Ensure policy routing table 100 is configured
    let _ = PolicyRoutingManager::setup_node_policy_routing(100, 0x1, 100, "wg0", Some("10.200.0.1"));

    loop {
        match watcher.scan_active_ports().await {
            Ok(current_ports) => {
                if current_ports != last_ports {
                    info!(
                        "[Docker Watcher] Discovered {} active game port(s). Reconciling kernel DNAT...",
                        current_ports.len()
                    );
                    for p in &current_ports {
                        info!(
                            "  → Port {}/{} (Container IP: {:?})",
                            p.port,
                            match p.protocol {
                                protocol::ProtocolType::Udp => "UDP",
                                _ => "TCP",
                            },
                            p.container_ip
                        );
                    }
                    if let Err(e) = NodeReconciler::reconcile_container_routes(virtual_ip, &current_ports) {
                        warn!("Failed to reconcile container routes: {:?}", e);
                    }
                    last_ports = current_ports;
                }
            }
            Err(e) => {
                debug!("Docker scan notice: {:?}", e);
            }
        }

        tokio::time::sleep(Duration::from_millis(2500)).await;
    }
}

fn show_node_status() -> Result<()> {
    println!("==========================================================");
    println!(" 🖥️  WireNet Pterodactyl Node Status");
    println!("==========================================================");

    // 1. Interface Stats
    let iface = telemetry::read_interface_stats("wg0", 0, 1.0);
    if iface.exists {
        println!(" [✓] Interface wg0       : UP");
        println!("     Total Packets       : {}", iface.total_packets);
        println!("     Data Transferred    : RX {} bytes │ TX {} bytes", iface.rx_bytes, iface.tx_bytes);
    } else {
        println!(" [!] Interface wg0       : DOWN / NOT CONFIGURED");
        println!("     Run 'wirenet-node setup' to configure the tunnel.");
    }

    // 2. WireGuard Peers
    let peers = telemetry::read_wireguard_peers("wg0");
    if peers.is_empty() {
        println!(" [!] WireGuard Peers     : No peers active on wg0");
    } else {
        for p in peers {
            println!(" [✓] Gateway Peer        : {}", p.endpoint);
            println!("     Latest Handshake    : {}", p.latest_handshake_ago);
            println!("     Peer Allowed IPs    : {}", p.allowed_ips.join(", "));
            println!("     Status              : {}", if p.is_online { "ONLINE (Active link)" } else { "Awaiting Handshake" });
        }
    }

    // 3. Routing Table 100
    let ip_rule = Command::new("ip").args(["rule", "show"]).output();
    if let Ok(out) = ip_rule {
        let s = String::from_utf8_lossy(&out.stdout);
        if s.contains("fwmark 0x1") && s.contains("lookup 100") {
            println!(" [✓] Policy Routing      : Table 100 + fwmark 0x1 ACTIVE");
        } else {
            println!(" [!] Policy Routing      : Table 100 rule not detected");
        }
    }

    // 4. Connected Real Player IPs
    let players = telemetry::scan_real_connections(&[]);
    println!(" Active Player Sessions  : {}", players.len());
    for p in players {
        println!("  → Real IP {}:{} ──► Port {} ({}) [{}]", p.client_ip, p.client_port, p.game_port, p.protocol, p.state);
    }

    println!("==========================================================");
    Ok(())
}

fn run_node_doctor() -> Result<()> {
    println!("==========================================================");
    println!(" 🩺 WireNet Node System Doctor");
    println!("==========================================================");

    // Check 1: IP Forwarding
    let fwd = fs::read_to_string("/proc/sys/net/ipv4/ip_forward").unwrap_or_default();
    if fwd.trim() == "1" {
        println!(" [✓] Kernel IPv4 Forwarding : ENABLED");
    } else {
        println!(" [!] Kernel IPv4 Forwarding : DISABLED (Fixing...)");
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.ip_forward=1"]).output();
    }

    // Check 2: Route Localnet
    let rln = fs::read_to_string("/proc/sys/net/ipv4/conf/all/route_localnet").unwrap_or_default();
    if rln.trim() == "1" {
        println!(" [✓] Kernel Route Localnet  : ENABLED");
    } else {
        println!(" [!] Kernel Route Localnet  : DISABLED (Fixing...)");
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.all.route_localnet=1"]).output();
    }

    // Check 3: WireGuard Tools
    if Command::new("which").arg("wg").output().map(|o| o.status.success()).unwrap_or(false) {
        println!(" [✓] WireGuard Toolchain    : INSTALLED");
    } else {
        println!(" [!] WireGuard Toolchain    : NOT FOUND (Run apt install wireguard-tools)");
    }

    // Check 4: Docker Socket Reachable
    let sock = Path::new("/var/run/docker.sock");
    if sock.exists() {
        println!(" [✓] Docker Socket          : PRESENT (/var/run/docker.sock)");
    } else {
        println!(" [!] Docker Socket          : NOT FOUND (Is Docker installed & running?)");
    }

    // Check 5: Policy Routing Table 100
    let rule_out = Command::new("ip").args(["rule", "show"]).output();
    let has_rule = rule_out.map(|o| String::from_utf8_lossy(&o.stdout).contains("100")).unwrap_or(false);
    if has_rule {
        println!(" [✓] Policy Routing Table100: CONFIGURED");
    } else {
        println!(" [!] Policy Routing Table100: MISSING (Run 'wirenet-node setup')");
    }

    println!("==========================================================");
    Ok(())
}

fn install_node_service() -> Result<()> {
    let unit = "[Unit]\n\
         Description=WireNet Pterodactyl Node Agent\n\
         After=network-online.target docker.service\n\
         Wants=network-online.target\n\n\
         [Service]\n\
         Type=simple\n\
         ExecStart=/usr/local/bin/wirenet-node run\n\
         Restart=always\n\
         RestartSec=3\n\n\
         [Install]\n\
         WantedBy=multi-user.target\n";

    fs::write("/etc/systemd/system/wirenet-node.service", unit)
        .context("writing /etc/systemd/system/wirenet-node.service")?;
    let _ = Command::new("systemctl").args(["daemon-reload"]).output();
    let _ = Command::new("systemctl").args(["enable", "--now", "wirenet-node.service"]).output();

    println!("==========================================================");
    println!(" [✓] wirenet-node.service installed and started!");
    println!(" Check status anytime with: systemctl status wirenet-node");
    println!("==========================================================");

    Ok(())
}
