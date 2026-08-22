use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub bind_ip: String,
    pub control_port: u16,
    pub auth_token: String,
    pub public_ports_start: u16,
    pub public_ports_end: u16,
    pub shield_mode: String,
    pub syn_rate_limit: u32,
    pub max_connections_per_ip: u32,
    pub enable_proxy_protocol: bool,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            bind_ip: "0.0.0.0".to_string(),
            control_port: 9000,
            auth_token: "wirenet_secret_token_default".to_string(),
            public_ports_start: 25565,
            public_ports_end: 25700,
            shield_mode: "standard".to_string(),
            syn_rate_limit: 25,
            max_connections_per_ip: 50,
            enable_proxy_protocol: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub node_id: String,
    pub node_name: String,
    pub gateway_endpoint: String,
    pub auth_token: String,
    pub docker_socket_path: PathBuf,
    pub auto_discover_docker: bool,
    pub static_ports: Vec<u16>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_id: "node-1".to_string(),
            node_name: "Pterodactyl Node 1".to_string(),
            gateway_endpoint: "10.200.0.1:9000".to_string(),
            auth_token: "wirenet_secret_token_default".to_string(),
            docker_socket_path: PathBuf::from("/var/run/docker.sock"),
            auto_discover_docker: true,
            static_ports: vec![25565, 25566, 25567],
        }
    }
}
