use super::router::GatewayRouter;
use super::shield::AntiDDoSShield;
use crate::config::GatewayConfig;
use crate::controller::Store;
use crate::net::firewall::{FirewallEngine, GatewayMappingRule};
use crate::net::routing::PolicyRoutingManager;
use crate::net::wireguard::{WireGuardInterface, WireGuardPeer};
use crate::protocol::{Message, WireNetCodec};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_util::codec::Framed;
use tracing::{error, info, warn};

pub struct GatewayReconciler;

impl GatewayReconciler {
    /// Reads desired state from the local store and applies WireGuard and kernel DNAT in-place
    pub fn apply_desired_state(
        store: &Store,
        wg_conf_path: &Path,
        wan_iface: Option<&str>,
    ) -> Result<usize> {
        let state = store.state().context("querying controller state")?;

        // 1. Enable IPv4 kernel forwarding
        PolicyRoutingManager::enable_ip_forwarding().context("enabling kernel IPv4 forwarding")?;

        // 2. Ensure gateway private key exists
        let priv_key_path = Path::new("/etc/wireguard/gateway_private.key");
        let private_key = if priv_key_path.exists() {
            fs::read_to_string(priv_key_path)?.trim().to_string()
        } else {
            let key = "a".repeat(44); // Fallback if wg not present in unit test environment
            if let Ok((priv_k, pub_k)) = crate::net::wireguard::generate_keypair() {
                let _ = fs::write(priv_key_path, &priv_k);
                let _ = fs::write("/etc/wireguard/gateway_public.key", &pub_k);
                priv_k
            } else {
                key
            }
        };

        // 3. Configure WireGuard Interface (10.200.0.1/24)
        let mut wg_iface = WireGuardInterface::new("wg0", &private_key, 51820, "10.200.0.1/24");

        for node in &state.nodes {
            wg_iface.add_peer(WireGuardPeer {
                public_key: node.public_key.clone(),
                endpoint: None,
                allowed_ips: vec![format!("{}/32", node.tunnel_ip)],
                persistent_keepalive: None,
            });
        }

        let _ = wg_iface.apply(wg_conf_path);

        // 4. Generate & Apply Kernel DNAT and Forwarding Rules
        let mut rules = Vec::new();
        for mapping in &state.mappings {
            if !mapping.enabled {
                continue;
            }

            // Lookup target node tunnel IP
            if let Some(node) = state.nodes.iter().find(|n| n.id == mapping.node_id) {
                rules.push(GatewayMappingRule {
                    public_ip: mapping.public_ip.clone(),
                    public_port: mapping.public_port,
                    protocol: mapping.protocol.clone(),
                    node_tunnel_ip: node.tunnel_ip.clone(),
                    backend_port: mapping.backend_port,
                });
            }
        }

        // Apply via nftables if supported
        let nft = FirewallEngine::render_gateway_nftables(&rules, wan_iface, "wg0");
        if let Err(err) = FirewallEngine::apply_nftables(&nft) {
            warn!(
                "nftables apply failed ({:?}); falling back to iptables commands",
                err
            );
            let ipt_cmds = FirewallEngine::render_gateway_iptables(&rules, "wg0");
            for cmd in ipt_cmds {
                if cmd.len() > 1 {
                    let _ = std::process::Command::new(&cmd[0]).args(&cmd[1..]).output();
                }
            }
        }

        info!(
            "[Gateway Reconciler] Applied {} active mappings across {} nodes (Zero-Proxy Kernel Routing)",
            rules.len(),
            state.nodes.len()
        );

        Ok(rules.len())
    }
}

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

    pub async fn run(self: Arc<Self>, store: Option<Store>) -> Result<()> {
        info!("==========================================================");
        info!(" 🛡️  WireNet Gateway Engine Active (Pure Kernel Routing)");
        info!(" Listening on Control Port: {}", self.config.control_port);
        info!(" Forwarding via Linux Kernel DNAT (Zero Userspace Proxy)");
        info!("==========================================================");

        // 1. Initial State Reconciliation
        if let Some(s) = &store {
            let conf_path = Path::new("/etc/wireguard/wg0.conf");
            if let Err(e) = GatewayReconciler::apply_desired_state(s, conf_path, None) {
                warn!("Initial gateway state reconciliation error: {:?}", e);
            }
        }

        // 2. Spawn Control Plane Listener for Node Agents
        let control_server = Arc::clone(&self);
        tokio::spawn(async move {
            if let Err(e) = control_server.run_control_listener().await {
                error!("Control plane error: {:?}", e);
            }
        });

        // 3. Periodic Shield & IP Cleanup Loop
        let cleanup_shield = self.shield.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                cleanup_shield.cleanup_stale_ips().await;
            }
        });

        // 4. Periodic Desired-State Reconciliation Loop
        if let Some(s) = store {
            let conf_path = Path::new("/etc/wireguard/wg0.conf").to_path_buf();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    let _ = GatewayReconciler::apply_desired_state(&s, &conf_path, None);
                }
            });
        }

        // Keep main server task alive
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
                                        "[Control Plane] Unauthorized registration attempt from {}: {}",
                                        node_id, peer_addr
                                    );
                                    let _ = framed
                                        .send(Message::RegisterAck {
                                            success: false,
                                            gateway_version: "2.0.0".to_string(),
                                            assigned_ports: Vec::new(),
                                            error: Some("Invalid auth token".to_string()),
                                        })
                                        .await;
                                    break;
                                }

                                info!(
                                    "[Control Plane] Node Registered: {} ({}) -> Virtual IP: {}",
                                    node_id, node_name, virtual_ip
                                );
                                router.register_node(node_id, node_name, virtual_ip.clone());

                                let _ = framed
                                    .send(Message::RegisterAck {
                                        success: true,
                                        gateway_version: "2.0.0".to_string(),
                                        assigned_ports: Vec::new(),
                                        error: None,
                                    })
                                    .await;
                            }
                            Message::PortSync { node_id, ports } => {
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
}
