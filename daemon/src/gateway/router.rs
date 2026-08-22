use crate::protocol::PortMapping;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Instant;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct NodeSession {
    pub node_id: String,
    pub node_name: String,
    pub virtual_ip: String,
    pub registered_at: Instant,
    pub last_heartbeat: Instant,
    pub port_mappings: Vec<PortMapping>,
}

#[derive(Clone, Default)]
pub struct GatewayRouter {
    // Port -> Node ID
    port_to_node: Arc<DashMap<u16, String>>,
    // Node ID -> Node Session Info
    nodes: Arc<DashMap<String, NodeSession>>,
}

impl GatewayRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_node(&self, node_id: String, node_name: String, virtual_ip: String) {
        self.nodes.insert(
            node_id.clone(),
            NodeSession {
                node_id,
                node_name,
                virtual_ip,
                registered_at: Instant::now(),
                last_heartbeat: Instant::now(),
                port_mappings: Vec::new(),
            },
        );
    }

    pub fn sync_node_ports(&self, node_id: &str, ports: Vec<PortMapping>) {
        if let Some(mut session) = self.nodes.get_mut(node_id) {
            // Remove old mappings
            for old in &session.port_mappings {
                self.port_to_node.remove(&old.port);
            }

            // Insert new mappings
            for port in &ports {
                self.port_to_node.insert(port.port, node_id.to_string());
            }

            session.port_mappings = ports;
        }
    }

    pub fn update_heartbeat(&self, node_id: &str) {
        if let Some(mut session) = self.nodes.get_mut(node_id) {
            session.last_heartbeat = Instant::now();
        }
    }

    pub fn get_target_node_ip(&self, port: u16) -> Option<String> {
        let node_id = self.port_to_node.get(&port)?;
        let session = self.nodes.get(&*node_id)?;
        Some(session.virtual_ip.clone())
    }

    #[allow(dead_code)]
    pub fn get_active_nodes(&self) -> Vec<NodeSession> {
        self.nodes.iter().map(|item| item.value().clone()).collect()
    }

    #[allow(dead_code)]
    pub fn get_total_mapped_ports(&self) -> usize {
        self.port_to_node.len()
    }
}
