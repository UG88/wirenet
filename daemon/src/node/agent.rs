use super::docker_watcher::DockerWatcher;
use crate::config::NodeConfig;
use crate::protocol::{Message, WireNetCodec};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use tracing::{debug, info, warn};

pub struct NodeAgent {
    config: NodeConfig,
    docker_watcher: DockerWatcher,
}

impl NodeAgent {
    pub fn new(config: NodeConfig) -> Self {
        let docker_watcher = DockerWatcher::new(config.docker_socket_path.clone());
        Self {
            config,
            docker_watcher,
        }
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        info!("==========================================================");
        info!(" 🚀  WireNet Node Agent Active: {}", self.config.node_name);
        info!(" Target Gateway Endpoint: {}", self.config.gateway_endpoint);
        info!("==========================================================");

        loop {
            match self.connect_and_sync().await {
                Ok(_) => {
                    info!("[Node Agent] Connection closed cleanly. Reconnecting in 3s...");
                }
                Err(e) => {
                    warn!("[Node Agent] Gateway connection lost ({:?}). Reconnecting in 5s...", e);
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn connect_and_sync(&self) -> Result<()> {
        info!("[Node Agent] Connecting to Gateway at {}...", self.config.gateway_endpoint);
        let stream = TcpStream::connect(&self.config.gateway_endpoint).await
            .with_context(|| format!("Could not connect to Gateway at {}", self.config.gateway_endpoint))?;

        let mut framed = Framed::new(stream, WireNetCodec::new());

        // 1. Register with Gateway
        let register_msg = Message::NodeRegister {
            node_id: self.config.node_id.clone(),
            node_name: self.config.node_name.clone(),
            virtual_ip: "10.200.0.2".to_string(),
            auth_token: self.config.auth_token.clone(),
        };

        framed.send(register_msg).await?;

        // 2. Wait for RegisterAck
        if let Some(Ok(Message::RegisterAck { success, gateway_version, error, .. })) = framed.next().await {
            if !success {
                return Err(anyhow::anyhow!("Gateway rejected registration: {:?}", error));
            }
            info!("[Node Agent] [✓] Successfully registered with Gateway v{}", gateway_version);
        }

        // 3. Heartbeat & Port Discovery Sync Loop
        let mut ticker = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    // Send Heartbeat
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;

                    framed.send(Message::Heartbeat {
                        node_id: self.config.node_id.clone(),
                        timestamp_ms: now_ms,
                        active_connections: 0,
                    }).await?;

                    // Scan and sync ports with kernel direct DNAT
                    if let Ok(ports) = self.docker_watcher.scan_active_ports().await {
                        for p in &ports {
                            if let Some(ref cip) = p.container_ip {
                                let dest = format!("{}:{}", cip, p.container_port);
                                let port_str = p.port.to_string();
                                let _ = std::process::Command::new("iptables")
                                    .args(["-t", "nat", "-I", "PREROUTING", "1", "-i", "wg0", "-p", "tcp", "--dport", &port_str, "-j", "DNAT", "--to-destination", &dest])
                                    .output();
                                let _ = std::process::Command::new("iptables")
                                    .args(["-t", "nat", "-I", "PREROUTING", "1", "-i", "wg0", "-p", "udp", "--dport", &port_str, "-j", "DNAT", "--to-destination", &dest])
                                    .output();
                                let _ = std::process::Command::new("iptables")
                                    .args(["-t", "nat", "-I", "PREROUTING", "1", "-d", "10.200.0.2", "-p", "tcp", "--dport", &port_str, "-j", "DNAT", "--to-destination", &dest])
                                    .output();
                                let _ = std::process::Command::new("iptables")
                                    .args(["-t", "nat", "-I", "PREROUTING", "1", "-d", "10.200.0.2", "-p", "udp", "--dport", &port_str, "-j", "DNAT", "--to-destination", &dest])
                                    .output();
                            }
                        }
                        framed.send(Message::PortSync {
                            node_id: self.config.node_id.clone(),
                            ports,
                        }).await?;
                    }
                }
                msg = framed.next() => {
                    match msg {
                        Some(Ok(Message::HeartbeatAck { .. })) => {
                            // Heartbeat confirmed
                        }
                        Some(Ok(other)) => {
                            debug!("[Node Agent] Received message: {:?}", other);
                        }
                        Some(Err(e)) => return Err(e),
                        None => return Ok(()),
                    }
                }
            }
        }
    }
}
