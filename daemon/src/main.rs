mod config;
pub mod controller;
pub mod dashboard_server;
mod gateway;
mod node;
mod ops;
mod protocol;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::{GatewayConfig, NodeConfig};
use gateway::GatewayServer;
use node::NodeAgent;
use ops::{
    DoctorManager, SetupManager, ShieldManager, StatusManager, UninstallManager, UpdateManager,
};
use std::sync::Arc;
use tui::TuiDashboard;

#[derive(Parser)]
#[command(name = "wirenet")]
#[command(author = "UG88 <untilgamer888@gmail.com>")]
#[command(version = "2.0.0")]
#[command(about = "High-Performance WireNet Tunnel & Anti-DDoS Daemon for Game Servers")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start WireNet Daemon (Auto-detects Gateway or Node role)
    Run,
    /// Setup WireNet WireGuard Tunnel & Services
    Setup {
        #[command(subcommand)]
        sub: SetupCommands,
    },
    /// Gateway VPS Commands (Public Ingress & Anti-DDoS Scrubbing)
    Gateway {
        #[command(subcommand)]
        sub: GatewayCommands,
    },
    /// Pterodactyl Node Agent Commands (Docker Auto-Discovery & Ingress)
    Node {
        #[command(subcommand)]
        sub: NodeCommands,
    },
    /// Launch Interactive Real-Time Terminal Dashboard
    Tui,
    /// Run 6-Point System Diagnostic Doctor & Self-Healing Scan
    Doctor,
    /// Display Current Shield, Tunnel & Connection Status
    Status,
    /// Configure Anti-DDoS Shielding Mode (standard, strict, off)
    Shield {
        #[arg(default_value = "standard")]
        mode: String,
    },
    /// Check for updates from GitHub
    CheckUpdate,
    /// 1-Click Self Update to Latest GitHub Version
    Update,
    /// Apply In-Place Real IP Routing Update (Preserves Existing Keys & Peers)
    Apply,
    /// Authorize and Persist a Node Peer (Gateway Hub)
    Peer {
        #[command(subcommand)]
        sub: PeerCommands,
    },
    /// 100% Deep Cleaner & Complete Uninstaller
    Uninstall,
    /// Optional Axum Control-Plane Web UI
    Dashboard {
        #[command(subcommand)]
        sub: DashboardCommands,
    },
    /// Local SQLite Desired-State Controller Store
    Controller {
        #[command(subcommand)]
        sub: ControllerCommands,
    },
}

#[derive(Subcommand)]
enum DashboardCommands {
    /// Run the authenticated Axum dashboard HTTP server
    Serve {
        /// SQLite desired-state database file path
        #[arg(long, default_value = "/etc/wirenet/desired_state.db")]
        database: std::path::PathBuf,

        /// Path to bearer token file
        #[arg(long, default_value = "/etc/wirenet/dashboard.token")]
        token_file: std::path::PathBuf,

        /// Listening address (loopback only unless --allow-remote)
        #[arg(long, default_value = "127.0.0.1:8080")]
        listen: std::net::SocketAddr,

        /// Permit binding to non-loopback address
        #[arg(long, default_value_t = false)]
        allow_remote: bool,
    },
    /// Generate a cryptographically secure 256-bit dashboard token
    Token {
        /// Path to save token file (0600 on unix)
        #[arg(long, default_value = "/etc/wirenet/dashboard.token")]
        token_file: std::path::PathBuf,
    },
    /// Install optional systemd service unit (wirenet-dashboard.service)
    Install {
        #[arg(long, default_value = "/etc/wirenet/desired_state.db")]
        database: std::path::PathBuf,

        #[arg(long, default_value = "/etc/wirenet/dashboard.token")]
        token_file: std::path::PathBuf,

        #[arg(long, default_value = "127.0.0.1:8080")]
        listen: std::net::SocketAddr,

        #[arg(long, default_value_t = false)]
        allow_remote: bool,
    },
    /// Enable and start wirenet-dashboard.service
    Enable,
    /// Disable and stop wirenet-dashboard.service
    Disable,
    /// Check wirenet-dashboard.service status
    ServiceStatus,
}

#[derive(Subcommand)]
enum ControllerCommands {
    /// Show current desired state (nodes, servers, mappings, bans)
    Show {
        #[arg(long, default_value = "/etc/wirenet/desired_state.db")]
        database: std::path::PathBuf,

        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Add an active node to desired state
    AddNode {
        #[arg(long, default_value = "/etc/wirenet/desired_state.db")]
        database: std::path::PathBuf,

        #[arg(long)]
        id: String,

        #[arg(long)]
        name: String,

        #[arg(long)]
        tunnel_ip: String,

        #[arg(long)]
        public_key: String,
    },
    /// Add a server to desired state
    AddServer {
        #[arg(long, default_value = "/etc/wirenet/desired_state.db")]
        database: std::path::PathBuf,

        #[arg(long)]
        id: String,

        #[arg(long)]
        node_id: String,

        #[arg(long)]
        customer_id: String,

        #[arg(long)]
        pterodactyl_id: String,
    },
    /// Reserve an ingress mapping in desired state (IP:port:protocol)
    AddMapping {
        #[arg(long, default_value = "/etc/wirenet/desired_state.db")]
        database: std::path::PathBuf,

        #[arg(long)]
        id: String,

        #[arg(long)]
        server_id: String,

        #[arg(long)]
        public_ip: String,

        #[arg(long)]
        public_port: u16,

        #[arg(long)]
        backend_port: u16,

        #[arg(long)]
        protocol: String,
    },
    /// Toggle mapping enabled state
    ToggleMapping {
        #[arg(long, default_value = "/etc/wirenet/desired_state.db")]
        database: std::path::PathBuf,

        #[arg(long)]
        id: String,

        #[arg(long)]
        enabled: bool,
    },
    /// Add an IP ban to desired state
    AddBan {
        #[arg(long, default_value = "/etc/wirenet/desired_state.db")]
        database: std::path::PathBuf,

        #[arg(long)]
        ip: String,

        #[arg(long)]
        reason: String,

        #[arg(long)]
        mapping_id: Option<String>,

        #[arg(long)]
        expires_in_secs: Option<u64>,
    },
    /// Remove an IP ban by ID from desired state
    RemoveBan {
        #[arg(long, default_value = "/etc/wirenet/desired_state.db")]
        database: std::path::PathBuf,

        #[arg(long)]
        id: i64,
    },
}

#[derive(Subcommand)]
enum PeerCommands {
    /// Add and permanently persist a Node peer public key on the Gateway
    Add {
        /// Node Public Key
        key: String,
        /// Node Virtual IP
        #[arg(default_value = "10.200.0.2")]
        ip: String,
    },
}

#[derive(Subcommand)]
enum SetupCommands {
    /// Install & Configure Gateway VPS (Hub)
    Gateway {
        #[arg(long, default_value_t = 25565)]
        start_port: u16,

        #[arg(long, default_value_t = 25700)]
        end_port: u16,
    },
    /// Install & Configure Pterodactyl Node VPS (Spoke)
    Node {
        #[arg(short, long)]
        gateway: String,

        #[arg(short = 'k', long)]
        gateway_key: String,
    },
}

#[derive(Subcommand)]
enum GatewayCommands {
    /// Start the Gateway Public Ingress & Scrubbing Engine
    Run {
        #[arg(short, long, default_value = "0.0.0.0")]
        bind: String,

        #[arg(short, long, default_value_t = 9000)]
        control_port: u16,

        #[arg(long, default_value = "wirenet_secret_token_default")]
        token: String,

        #[arg(long, default_value = "standard")]
        shield: String,

        #[arg(long, default_value_t = 25565)]
        start_port: u16,

        #[arg(long, default_value_t = 25700)]
        end_port: u16,
    },
}

#[derive(Subcommand)]
enum NodeCommands {
    /// Start the Node Agent & Docker Container Watcher
    Run {
        #[arg(short, long, default_value = "10.200.0.1:9000")]
        gateway: String,

        #[arg(long, default_value = "node-1")]
        node_id: String,

        #[arg(long, default_value = "Pterodactyl Node 1")]
        name: String,

        #[arg(long, default_value = "wirenet_secret_token_default")]
        token: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wirenet_daemon=info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        None => {
            // Default when running 'wirenet' with no args -> Launch Dashboard
            TuiDashboard::run()?;
        }
        Some(Commands::Run) => {
            let is_gateway = std::fs::read_to_string("/etc/wireguard/wg0.conf")
                .map(|c| c.contains("10.200.0.1/24"))
                .unwrap_or(false)
                || std::path::Path::new("/etc/wireguard/gateway_private.key").exists();

            if is_gateway {
                let config = GatewayConfig::default();
                let gateway = Arc::new(GatewayServer::new(config));
                gateway.run().await?;
            } else {
                let config = NodeConfig::default();
                let agent = Arc::new(NodeAgent::new(config));
                agent.run().await?;
            }
        }
        Some(Commands::Setup { sub }) => match sub {
            SetupCommands::Gateway {
                start_port,
                end_port,
            } => {
                SetupManager::setup_gateway(start_port, end_port)?;
            }
            SetupCommands::Node {
                gateway,
                gateway_key,
            } => {
                SetupManager::setup_node(&gateway, &gateway_key)?;
            }
        },
        Some(Commands::Gateway { sub }) => match sub {
            GatewayCommands::Run {
                bind,
                control_port,
                token,
                shield,
                start_port,
                end_port,
            } => {
                let config = GatewayConfig {
                    bind_ip: bind,
                    control_port,
                    auth_token: token,
                    shield_mode: shield,
                    public_ports_start: start_port,
                    public_ports_end: end_port,
                    ..Default::default()
                };

                let gateway = Arc::new(GatewayServer::new(config));
                gateway.run().await?;
            }
        },
        Some(Commands::Node { sub }) => match sub {
            NodeCommands::Run {
                gateway,
                node_id,
                name,
                token,
            } => {
                let config = NodeConfig {
                    node_id,
                    node_name: name,
                    gateway_endpoint: gateway,
                    auth_token: token,
                    ..Default::default()
                };

                let agent = Arc::new(NodeAgent::new(config));
                agent.run().await?;
            }
        },
        Some(Commands::Tui) => {
            TuiDashboard::run()?;
        }
        Some(Commands::Doctor) => {
            DoctorManager::run_diagnostics()?;
        }
        Some(Commands::Status) => {
            StatusManager::show_status()?;
        }
        Some(Commands::Shield { mode }) => {
            ShieldManager::set_mode(&mode)?;
        }
        Some(Commands::CheckUpdate) => {
            UpdateManager::check_update()?;
        }
        Some(Commands::Update) => {
            UpdateManager::self_update()?;
        }
        Some(Commands::Apply) => {
            SetupManager::apply_real_ip_routing()?;
        }
        Some(Commands::Peer { sub }) => match sub {
            PeerCommands::Add { key, ip } => {
                SetupManager::add_node_peer(&key, &ip)?;
            }
        },
        Some(Commands::Uninstall) => {
            UninstallManager::deep_uninstall()?;
        }
        Some(Commands::Dashboard { sub }) => match sub {
            DashboardCommands::Serve {
                database,
                token_file,
                listen,
                allow_remote,
            } => {
                dashboard_server::validate_listen(listen, allow_remote)?;
                let token = dashboard_server::load_token(&token_file)?;
                let store = controller::Store::open(&database)?;
                println!("Starting WireNet dashboard on http://{listen}");
                println!("Backend database: {}", database.display());
                dashboard_server::serve(store, listen, token).await?;
            }
            DashboardCommands::Token { token_file } => {
                let token = dashboard_server::create_token(&token_file)?;
                println!("Generated dashboard token at: {}", token_file.display());
                println!("Token: {token}");
            }
            DashboardCommands::Install {
                database,
                token_file,
                listen,
                allow_remote,
            } => {
                dashboard_server::install_service(&database, &token_file, listen, allow_remote)?;
                println!("Installed /etc/systemd/system/wirenet-dashboard.service");
            }
            DashboardCommands::Enable => {
                dashboard_server::set_enabled(true)?;
                println!("Enabled and started wirenet-dashboard.service");
            }
            DashboardCommands::Disable => {
                dashboard_server::set_enabled(false)?;
                println!("Disabled and stopped wirenet-dashboard.service");
            }
            DashboardCommands::ServiceStatus => {
                let status = dashboard_server::service_status()?;
                println!("wirenet-dashboard.service is: {status}");
            }
        },
        Some(Commands::Controller { sub }) => match sub {
            ControllerCommands::Show { database, json } => {
                let store = controller::Store::open(&database)?;
                let state = store.state()?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&state)?);
                } else {
                    println!("=== WireNet Desired State ({}) ===", database.display());
                    println!("Nodes ({}):", state.nodes.len());
                    for n in &state.nodes {
                        let key_snippet = if n.public_key.len() > 12 {
                            &n.public_key[..12]
                        } else {
                            &n.public_key
                        };
                        println!(
                            "  - [{}] {} (tunnel: {}, pubkey: {}...)",
                            n.id, n.name, n.tunnel_ip, key_snippet
                        );
                    }
                    println!("Servers ({}):", state.servers.len());
                    for s in &state.servers {
                        println!(
                            "  - [{}] node: {}, customer: {}, ptero: {}, state: {}",
                            s.id, s.node_id, s.customer_id, s.pterodactyl_id, s.state
                        );
                    }
                    println!("Mappings ({}):", state.mappings.len());
                    for m in &state.mappings {
                        let status = if m.enabled { "enabled" } else { "disabled" };
                        println!(
                            "  - [{}] {}:{}/{} -> node {} port {} ({})",
                            m.id,
                            m.public_ip,
                            m.public_port,
                            m.protocol,
                            m.node_id,
                            m.backend_port,
                            status
                        );
                    }
                    println!("Bans ({}):", state.bans.len());
                    for b in &state.bans {
                        println!(
                            "  - [id={}] ip: {}, reason: {}, mapping: {:?}",
                            b.id, b.ip, b.reason, b.mapping_id
                        );
                    }
                }
            }
            ControllerCommands::AddNode {
                database,
                id,
                name,
                tunnel_ip,
                public_key,
            } => {
                let store = controller::Store::open(&database)?;
                store.add_node(
                    controller::NodeInput {
                        id,
                        name,
                        tunnel_ip,
                        public_key,
                    },
                    "cli",
                )?;
                println!("Node added to desired state.");
            }
            ControllerCommands::AddServer {
                database,
                id,
                node_id,
                customer_id,
                pterodactyl_id,
            } => {
                let store = controller::Store::open(&database)?;
                store.add_server(
                    controller::ServerInput {
                        id,
                        node_id,
                        customer_id,
                        pterodactyl_id,
                    },
                    "cli",
                )?;
                println!("Server registered in desired state.");
            }
            ControllerCommands::AddMapping {
                database,
                id,
                server_id,
                public_ip,
                public_port,
                backend_port,
                protocol,
            } => {
                let store = controller::Store::open(&database)?;
                store.add_mapping(
                    controller::MappingInput {
                        id,
                        server_id,
                        public_ip,
                        public_port,
                        backend_port,
                        protocol,
                    },
                    "cli",
                )?;
                println!(
                    "Mapping reserved in desired state. (Apply remains explicit via network reconciler)"
                );
            }
            ControllerCommands::ToggleMapping {
                database,
                id,
                enabled,
            } => {
                let store = controller::Store::open(&database)?;
                store.toggle_mapping(&id, enabled, "cli")?;
                println!("Mapping {id} enabled state set to: {enabled}");
            }
            ControllerCommands::AddBan {
                database,
                ip,
                reason,
                mapping_id,
                expires_in_secs,
            } => {
                let store = controller::Store::open(&database)?;
                let expires_at = expires_in_secs.map(|s| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64
                        + s as i64
                });
                store.add_ban(
                    controller::BanInput {
                        ip,
                        reason,
                        mapping_id,
                        expires_at,
                    },
                    "cli",
                )?;
                println!("Ban recorded in desired state.");
            }
            ControllerCommands::RemoveBan { database, id } => {
                let store = controller::Store::open(&database)?;
                store.remove_ban(id, "cli")?;
                println!("Ban {id} removed.");
            }
        },
    }

    Ok(())
}
