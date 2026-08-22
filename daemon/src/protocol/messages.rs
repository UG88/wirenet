use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtocolType {
    Tcp,
    Udp,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub port: u16,
    pub protocol: ProtocolType,
    pub container_ip: Option<String>,
    pub container_port: u16,
    pub server_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // Node -> Gateway
    NodeRegister {
        node_id: String,
        node_name: String,
        virtual_ip: String,
        auth_token: String,
    },
    PortSync {
        node_id: String,
        ports: Vec<PortMapping>,
    },
    Heartbeat {
        node_id: String,
        timestamp_ms: u64,
        active_connections: u32,
    },

    // Gateway -> Node
    RegisterAck {
        success: bool,
        gateway_version: String,
        assigned_ports: Vec<u16>,
        error: Option<String>,
    },
    HeartbeatAck {
        timestamp_ms: u64,
    },
    ShieldStatusUpdate {
        mode: String,
        syn_rate_limit: u32,
    },

    // Ingress Control
    NewConnectionNotification {
        session_id: u64,
        client_addr: SocketAddr,
        target_port: u16,
    },
    ConnectionClosed {
        session_id: u64,
    },
}
