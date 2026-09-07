use super::docker_watcher::DockerWatcher;
use crate::config::NodeConfig;
use crate::net::firewall::{FirewallEngine, NodeMappingRule};
use crate::net::routing::PolicyRoutingManager;
use crate::net::wireguard::{WireGuardInterface, WireGuardPeer};
use crate::protocol::{Message, PortMapping, WireNetCodec};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use tracing::{debug, info, warn};

pub struct NodeReconciler;

impl NodeReconciler {
    /// Configures the WireGuard tunnel and policy routing table 100 (symmetric reply path)
    pub fn setup_network_tunnel(
        gateway_endpoint: &str,
        gateway_pub_key: &str,
        node_virtual_ip: &str,
        node_priv_key: &str,
        conf_path: &Path,
    ) -> Result<()> {
        info!("[Node Reconciler] Configuring WireGuard tunnel dev wg0 (Table = off)...");

        let mut wg_iface = WireGuardInterface::new(
            "wg0",
            node_priv_key,
            51820,
            &format!("{}/32", node_virtual_ip),
        )
        .with_table("off");

        wg_iface.add_peer(WireGuardPeer {
            public_key: gateway_pub_key.trim().to_string(),
            endpoint: Some(gateway_endpoint.trim().to_string()),
            allowed_ips: vec!["0.0.0.0/0".to_string()],
            persistent_keepalive: Some(25),
        });

        let _ = wg_iface.apply(conf_path);

        // Configure Linux kernel policy routing table 100 with fwmark 0x1
        info!("[Node Reconciler] Configuring policy routing table 100 & fwmark 0x1...");
        PolicyRoutingManager::setup_node_policy_routing(100, 0x1, 100, "wg0", Some("10.200.0.1"))
            .context("configuring policy routing table 100")?;

        Ok(())
    }

    /// Reconciles discovered container endpoints into conntrack return marks and direct container DNAT
    pub fn reconcile_container_routes(
        node_tunnel_ip: &str,
        active_ports: &[PortMapping],
    ) -> Result<()> {
        let mut rules = Vec::new();

        for p in active_ports {
            let container_ip = p.container_ip.clone().unwrap_or_else(|| "127.0.0.1".into());
            let proto = match p.protocol {
                crate::protocol::ProtocolType::Udp => "udp",
                _ => "tcp",
            };

            rules.push(NodeMappingRule {
                node_tunnel_ip: node_tunnel_ip.to_string(),
                backend_port: p.port,
                protocol: proto.to_string(),
                container_ip,
                container_port: p.port,
            });

            // If protocol is Both, also add UDP
            if matches!(p.protocol, crate::protocol::ProtocolType::Both) {
                rules.push(NodeMappingRule {
                    node_tunnel_ip: node_tunnel_ip.to_string(),
                    backend_port: p.port,
                    protocol: "udp".to_string(),
                    container_ip: p.container_ip.clone().unwrap_or_else(|| "127.0.0.1".into()),
                    container_port: p.port,
                });
            }
        }

        // Apply via nftables
        let nft = FirewallEngine::render_node_nftables(&rules, "wg0");
        if let Err(err) = FirewallEngine::apply_nftables(&nft) {
            warn!(
                "nftables apply failed ({:?}); falling back to iptables commands",
                err
            );
            let ipt_cmds = FirewallEngine::render_node_iptables(&rules, "wg0");
            for cmd in ipt_cmds {
                if cmd.len() > 1 {
                    let _ = std::process::Command::new(&cmd[0]).args(&cmd[1..]).output();
                }
            }
        }

        debug!(
            "[Node Reconciler] Reconciled {} container routes on {}",
            rules.len(),
            node_tunnel_ip
        );

        Ok(())
    }
}

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
        info!(" Mode: Transparent Kernel Return Routing (Table 100/fwmark 0x1)");
        info!("==========================================================");

        // 1. Initial Policy Routing & Sysctls
        let _ = PolicyRoutingManager::enable_ip_forwarding();
        let _ = PolicyRoutingManager::set_rp_filter("all", 2);
        let _ = PolicyRoutingManager::set_rp_filter("default", 2);
        let _ = PolicyRoutingManager::set_rp_filter("wg0", 0);
        let _ = PolicyRoutingManager::setup_node_policy_routing(
            100,
            0x1,
            100,
            "wg0",
            Some("10.200.0.1"),
        );

        // 2. Continuous Gateway Sync & Container Watcher Loop
        loop {
            match self.connect_and_sync().await {
                Ok(_) => {
                    info!("[Node Agent] Connection closed cleanly. Reconnecting in 3s...");
                }
                Err(e) => {
                    warn!(
                        "[Node Agent] Gateway connection lost ({:?}). Reconnecting in 5s...",
                        e
                    );
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn connect_and_sync(&self) -> Result<()> {
        info!(
            "[Node Agent] Connecting to Gateway Control Plane at {}...",
            self.config.gateway_endpoint
        );
        let stream = TcpStream::connect(&self.config.gateway_endpoint)
            .await
            .with_context(|| {
                format!(
                    "Could not connect to Gateway at {}",
                    self.config.gateway_endpoint
                )
            })?;

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
        if let Some(Ok(Message::RegisterAck {
            success,
            gateway_version,
            error,
            ..
        })) = framed.next().await
        {
            if !success {
                return Err(anyhow::anyhow!(
                    "Gateway rejected registration: {:?}",
                    error
                ));
            }
            info!(
                "[Node Agent] [✓] Successfully registered with Gateway v{}",
                gateway_version
            );
        }

        // 3. Heartbeat & Container Discovery Sync Loop
        let mut ticker = tokio::time::interval(Duration::from_secs(5));
        let mut last_reported_ports: Vec<PortMapping> = Vec::new();

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

                    // Scan Docker / Local container ports
                    match self.docker_watcher.scan_active_ports().await {
                        Ok(active_ports) => {
                            // Reconcile local container DNAT and conntrack routing rules
                            let _ = NodeReconciler::reconcile_container_routes("10.200.0.2", &active_ports);

                            // Send port sync to Gateway if changed
                            if active_ports != last_reported_ports {
                                info!(
                                    "[Node Agent] Syncing {} active game container ports to Gateway...",
                                    active_ports.len()
                                );
                                framed.send(Message::PortSync {
                                    node_id: self.config.node_id.clone(),
                                    ports: active_ports.clone(),
                                }).await?;
                                last_reported_ports = active_ports;
                            }
                        }
                        Err(e) => {
                            warn!("[Node Agent] Port discovery scan error: {:?}", e);
                        }
                    }
                }

                msg = framed.next() => {
                    match msg {
                        Some(Ok(Message::HeartbeatAck { .. })) => {
                            debug!("[Node Agent] Heartbeat acknowledged");
                        }
                        Some(Ok(other)) => {
                            debug!("[Node Agent] Received message from gateway: {:?}", other);
                        }
                        Some(Err(e)) => {
                            warn!("[Node Agent] Frame decode error: {:?}", e);
                            break;
                        }
                        None => {
                            info!("[Node Agent] Gateway closed the control stream");
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
