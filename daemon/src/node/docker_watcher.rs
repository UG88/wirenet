use crate::protocol::{PortMapping, ProtocolType};
use anyhow::Result;
use std::path::PathBuf;

#[allow(dead_code)]
pub struct DockerWatcher {
    socket_path: PathBuf,
}

impl DockerWatcher {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    /// Scans currently running Docker containers for Minecraft/Game port allocations
    pub async fn scan_active_ports(&self) -> Result<Vec<PortMapping>> {
        // 1. First priority: Direct Docker CLI inspect
        let cli_ports = self.scan_via_docker_cli();
        if !cli_ports.is_empty() {
            return Ok(cli_ports);
        }

        // 2. Second priority: Docker Unix domain socket inspect
        #[cfg(unix)]
        {
            if self.socket_path.exists() {
                if let Ok(ports) = self.scan_via_unix_socket().await {
                    if !ports.is_empty() {
                        return Ok(ports);
                    }
                }
            }
        }

        // 3. Fallback: Check local listening ports via default range
        Ok(self.scan_listening_ports().await)
    }

    fn scan_via_docker_cli(&self) -> Vec<PortMapping> {
        let mut mappings = Vec::new();
        let out = std::process::Command::new("docker")
            .args(["ps", "--format", "{{.ID}}\t{{.Ports}}\t{{.Names}}"])
            .output();

        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout);
            for line in s.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() < 2 {
                    continue;
                }
                let container_id = parts[0];
                let ports_str = parts[1];
                let name = parts.get(2).map(|&n| n.to_string());

                // Extract container internal IP address
                let ip_out = std::process::Command::new("docker")
                    .args([
                        "inspect",
                        container_id,
                        "--format",
                        "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}",
                    ])
                    .output();
                let container_ip = match ip_out {
                    Ok(io) => {
                        let ip_str = String::from_utf8_lossy(&io.stdout).trim().to_string();
                        if !ip_str.is_empty() {
                            Some(ip_str)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                for port_part in ports_str.split(',') {
                    let trimmed = port_part.trim();
                    if let Some(arrow_idx) = trimmed.find("->") {
                        let host_side = &trimmed[..arrow_idx];
                        let container_side = &trimmed[arrow_idx + 2..];

                        let port_num = host_side
                            .split(':')
                            .next_back()
                            .and_then(|p| p.parse::<u16>().ok())
                            .unwrap_or(0);

                        let priv_port = container_side
                            .split('/')
                            .next()
                            .and_then(|p| p.parse::<u16>().ok())
                            .unwrap_or(port_num);

                        let proto = if container_side.contains("/udp") {
                            ProtocolType::Udp
                        } else if container_side.contains("/tcp") {
                            ProtocolType::Tcp
                        } else {
                            ProtocolType::Both
                        };

                        if port_num >= 1024
                            && !mappings.iter().any(|m: &PortMapping| m.port == port_num)
                        {
                            mappings.push(PortMapping {
                                port: port_num,
                                protocol: proto,
                                container_ip: container_ip.clone(),
                                container_port: priv_port,
                                server_name: name.clone(),
                            });
                        }
                    }
                }
            }
        }
        mappings
    }

    #[cfg(unix)]
    async fn scan_via_unix_socket(&self) -> Result<Vec<PortMapping>> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::UnixStream;

        let mut stream = UnixStream::connect(&self.socket_path).await?;
        let request = "GET /containers/json HTTP/1.1\r\nHost: docker\r\n\r\n";
        stream.write_all(request.as_bytes()).await?;

        let mut response = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = stream.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            response.extend_from_slice(&buf[..n]);
            if response.windows(4).any(|w| w == b"\r\n\r\n") && response.len() > 500 {
                break;
            }
        }

        // Parse JSON container payload
        let response_str = String::from_utf8_lossy(&response);
        let mut mappings = Vec::new();

        if let Some(json_start) = response_str.find('[') {
            let json_body = &response_str[json_start..];
            if let Ok(containers) = serde_json::from_str::<serde_json::Value>(json_body) {
                if let Some(array) = containers.as_array() {
                    for c in array {
                        let name = c["Names"]
                            .as_array()
                            .and_then(|n| n.first())
                            .and_then(|n| n.as_str())
                            .map(|s| s.trim_start_matches('/').to_string());

                        let container_ip = c["NetworkSettings"]["Networks"]
                            .as_object()
                            .and_then(|nets| nets.values().next())
                            .and_then(|net| net["IPAddress"].as_str())
                            .filter(|ip| !ip.is_empty())
                            .map(|s| s.to_string());

                        if let Some(ports) = c["Ports"].as_array() {
                            for p in ports {
                                let public_port = p["PublicPort"].as_u64().unwrap_or(0) as u16;
                                let private_port = p["PrivatePort"].as_u64().unwrap_or(0) as u16;
                                let proto = match p["Type"].as_str().unwrap_or("tcp") {
                                    "udp" => ProtocolType::Udp,
                                    _ => ProtocolType::Tcp,
                                };

                                if public_port >= 1024 {
                                    mappings.push(PortMapping {
                                        port: public_port,
                                        protocol: proto,
                                        container_ip: container_ip.clone(),
                                        container_port: private_port,
                                        server_name: name.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(mappings)
    }

    /// ponytail: inspects authentic kernel listening sockets (/proc/net/tcp, /proc/net/udp) without fake/synthetic fallbacks
    async fn scan_listening_ports(&self) -> Vec<PortMapping> {
        #[allow(unused_mut)]
        let mut mappings = Vec::new();
        #[cfg(unix)]
        {
            // Inspect /proc/net/tcp for TCP_LISTEN (state 0A)
            if let Ok(content) = std::fs::read_to_string("/proc/net/tcp") {
                for line in content.lines().skip(1) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let local_addr = parts[1];
                        let state = parts[3];
                        if state == "0A" {
                            if let Some((_ip, port)) =
                                crate::net::telemetry::parse_hex_socket_addr(local_addr)
                            {
                                if is_game_port(port)
                                    && !mappings.iter().any(|m: &PortMapping| m.port == port)
                                {
                                    mappings.push(PortMapping {
                                        port,
                                        protocol: ProtocolType::Tcp,
                                        container_ip: Some("127.0.0.1".to_string()),
                                        container_port: port,
                                        server_name: Some(format!("Host Game Service (:{})", port)),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // Inspect /proc/net/udp for listening UDP sockets
            if let Ok(content) = std::fs::read_to_string("/proc/net/udp") {
                for line in content.lines().skip(1) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let local_addr = parts[1];
                        if let Some((_ip, port)) =
                            crate::net::telemetry::parse_hex_socket_addr(local_addr)
                        {
                            if is_game_port(port) {
                                if let Some(existing) = mappings.iter_mut().find(|m| m.port == port)
                                {
                                    existing.protocol = ProtocolType::Both;
                                } else if !mappings.iter().any(|m: &PortMapping| m.port == port) {
                                    mappings.push(PortMapping {
                                        port,
                                        protocol: ProtocolType::Udp,
                                        container_ip: Some("127.0.0.1".to_string()),
                                        container_port: port,
                                        server_name: Some(format!("Host Game Service (:{})", port)),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        mappings
    }
}

#[allow(dead_code)]
fn is_game_port(port: u16) -> bool {
    if port == 22
        || port == 53
        || port == 80
        || port == 443
        || port == 8080
        || port == 9000
        || port == 51820
    {
        return false;
    }
    (25565..=25700).contains(&port)
        || port == 19132
        || port == 24454
        || (27015..=27020).contains(&port)
        || (7777..=7780).contains(&port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_game_port() {
        assert!(is_game_port(25565)); // Minecraft Java standard
        assert!(is_game_port(19132)); // Bedrock standard
        assert!(is_game_port(24454)); // Simple Voice Chat
        assert!(is_game_port(27015)); // Source engine
        assert!(!is_game_port(22)); // SSH
        assert!(!is_game_port(80)); // HTTP
        assert!(!is_game_port(51820)); // WireGuard
        assert!(!is_game_port(9000)); // Control plane
    }
}
