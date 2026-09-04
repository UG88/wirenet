use super::router::GatewayRouter;
use super::shield::AntiDDoSShield;
use crate::config::GatewayConfig;
use crate::protocol::{Message, WireNetCodec};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
#[allow(unused_imports)]
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;
use tracing::{error, info, warn};

pub struct GatewayServer {
    config: GatewayConfig,
    shield: AntiDDoSShield,
    router: GatewayRouter,
}

impl GatewayServer {
    pub fn new(config: GatewayConfig) -> Self {
        let shield = AntiDDoSShield::new(&config.shield_mode, config.syn_rate_limit);
        let router = GatewayRouter::new();

        Self {
            config,
            shield,
            router,
        }
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        info!("==========================================================");
        info!(" 🛡️  WireNet Gateway Engine Active");
        info!(" Listening on Control Port: {}", self.config.control_port);
        info!(
            " Public Game Ingress Range: {}-{}",
            self.config.public_ports_start, self.config.public_ports_end
        );
        info!("==========================================================");

        // 1. Spawn Control Plane Listener
        let control_server = Arc::clone(&self);
        tokio::spawn(async move {
            if let Err(e) = control_server.run_control_listener().await {
                error!("Control plane error: {:?}", e);
            }
        });

        // 2. Spawn Public Ingress TCP Listeners for each configured port
        for port in self.config.public_ports_start..=self.config.public_ports_end {
            let ingress_server = Arc::clone(&self);
            tokio::spawn(async move {
                ingress_server.run_public_tcp_listener(port).await;
            });
        }

        // 3. Periodic Shield & IP Cleanup Loop
        let cleanup_shield = self.shield.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                cleanup_shield.cleanup_stale_ips().await;
            }
        });

        // Keep running
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    }

    async fn run_control_listener(&self) -> Result<()> {
        let addr = format!("{}:{}", self.config.bind_ip, self.config.control_port);
        let listener = TcpListener::bind(&addr)
            .await
            .with_context(|| format!("Failed to bind control plane listener on {}", addr))?;

        info!(
            "[Control Plane] Listening for Node Agent connections on {}",
            addr
        );

        loop {
            let (socket, peer_addr) = listener.accept().await?;
            let router = self.router.clone();
            let auth_token = self.config.auth_token.clone();

            tokio::spawn(async move {
                let mut framed = Framed::new(socket, WireNetCodec::new());
                info!(
                    "[Control Plane] Incoming connection from Node Agent: {}",
                    peer_addr
                );

                while let Some(msg_res) = framed.next().await {
                    match msg_res {
                        Ok(msg) => match msg {
                            Message::NodeRegister {
                                node_id,
                                node_name,
                                virtual_ip,
                                auth_token: token,
                            } => {
                                if token != auth_token {
                                    warn!(
                                        "[Control Plane] Node {} authentication failed!",
                                        node_id
                                    );
                                    let _ = framed
                                        .send(Message::RegisterAck {
                                            success: false,
                                            gateway_version: env!("CARGO_PKG_VERSION").to_string(),
                                            assigned_ports: vec![],
                                            error: Some("Invalid authentication token".to_string()),
                                        })
                                        .await;
                                    break;
                                }

                                info!(
                                    "[Control Plane] Node Registered: {} ({}) IP: {}",
                                    node_name, node_id, virtual_ip
                                );
                                router.register_node(node_id.clone(), node_name, virtual_ip);

                                let _ = framed
                                    .send(Message::RegisterAck {
                                        success: true,
                                        gateway_version: env!("CARGO_PKG_VERSION").to_string(),
                                        assigned_ports: vec![],
                                        error: None,
                                    })
                                    .await;
                            }
                            Message::PortSync { node_id, ports } => {
                                info!(
                                    "[Control Plane] Node {} synced {} active game ports",
                                    node_id,
                                    ports.len()
                                );
                                router.sync_node_ports(&node_id, ports);
                            }
                            Message::Heartbeat {
                                node_id,
                                timestamp_ms,
                                ..
                            } => {
                                router.update_heartbeat(&node_id);
                                let _ = framed.send(Message::HeartbeatAck { timestamp_ms }).await;
                            }
                            _ => {}
                        },
                        Err(e) => {
                            warn!("[Control Plane] Node stream error: {:?}", e);
                            break;
                        }
                    }
                }
            });
        }
    }

    async fn run_public_tcp_listener(&self, port: u16) {
        let addr = format!("{}:{}", self.config.bind_ip, port);
        let listener = match TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(_) => {
                // Port might be in use or reserved, skip silently
                return;
            }
        };

        loop {
            let (mut client_socket, client_addr) = match listener.accept().await {
                Ok(res) => res,
                Err(_) => continue,
            };

            // 1. Anti-DDoS rate-limit check
            if !self.shield.check_connection(client_addr.ip()) {
                // Drop connection instantly without allocating resources
                drop(client_socket);
                continue;
            }

            // 2. Find target backend node virtual IP (default 10.200.0.2)
            let target_node_ip = self
                .router
                .get_target_node_ip(port)
                .unwrap_or_else(|| "10.200.0.2".to_string());

            let shield = self.shield.clone();

            tokio::spawn(async move {
                let target_addr = format!("{}:{}", target_node_ip, port);
                if let Ok(mut backend_socket) = TcpStream::connect(&target_addr).await {
                    // Bidirectional zero-copy stream piping
                    let _ = tokio::io::copy_bidirectional(&mut client_socket, &mut backend_socket)
                        .await;
                }
                shield.connection_closed(client_addr.ip());
            });
        }
    }
}
