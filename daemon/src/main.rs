mod config;
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
use ops::{DoctorManager, SetupManager, ShieldManager, StatusManager, UninstallManager, UpdateManager};
use std::sync::Arc;
use tui::TuiDashboard;

#[derive(Parser)]
#[command(name = "wirenet")]
#[command(author = "UG88 <untilgamer888@gmail.com>")]
#[command(version = "0.1.0")]
#[command(about = "High-Performance WireNet Tunnel & Anti-DDoS Daemon for Game Servers")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
    /// 100% Deep Cleaner & Complete Uninstaller
    Uninstall,
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
        Commands::Setup { sub } => match sub {
            SetupCommands::Gateway { start_port, end_port } => {
                SetupManager::setup_gateway(start_port, end_port)?;
            }
            SetupCommands::Node { gateway, gateway_key } => {
                SetupManager::setup_node(&gateway, &gateway_key)?;
            }
        },
        Commands::Gateway { sub } => match sub {
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
        Commands::Node { sub } => match sub {
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
        Commands::Tui => {
            TuiDashboard::run()?;
        }
        Commands::Doctor => {
            DoctorManager::run_diagnostics()?;
        }
        Commands::Status => {
            StatusManager::show_status()?;
        }
        Commands::Shield { mode } => {
            ShieldManager::set_mode(&mode)?;
        }
        Commands::CheckUpdate => {
            UpdateManager::check_update()?;
        }
        Commands::Update => {
            UpdateManager::self_update()?;
        }
        Commands::Uninstall => {
            UninstallManager::deep_uninstall()?;
        }
    }

    Ok(())
}
