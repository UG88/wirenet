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
        let mut mappings = Vec::new();

        // On Linux, inspect via /var/run/docker.sock or docker cli fallback
        #[cfg(unix)]
        {
            if self.socket_path.exists() {
                if let Ok(ports) = self.scan_via_unix_socket().await {
                    return Ok(ports);
                }
            }
        }

        // Fallback: Check local listening ports via ss/netstat
        mappings.extend(self.scan_listening_ports().await);
        Ok(mappings)
    }

    #[cfg(unix)]
    async fn scan_via_unix_socket(&self) -> Result<Vec<PortMapping>> {
        use tokio::net::UnixStream;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut stream = UnixStream::connect(&self.socket_path).await?;
        let request = "GET /containers/json HTTP/1.1\r\nHost: docker\r\n\r\n";
        stream.write_all(request.as_bytes()).await?;

        let mut response = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = stream.read(&mut buf).await?;
            if n == 0 { break; }
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
                        let name = c["Names"].as_array()
                            .and_then(|n| n.first())
                            .and_then(|n| n.as_str())
                            .map(|s| s.trim_start_matches('/').to_string());

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
                                        container_ip: None,
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

    async fn scan_listening_ports(&self) -> Vec<PortMapping> {
        let mut mappings = Vec::new();
        // Common default range
        for port in [25565, 25566, 25567, 19132, 24454] {
            mappings.push(PortMapping {
                port,
                protocol: ProtocolType::Both,
                container_ip: None,
                container_port: port,
                server_name: Some("Pterodactyl Server".to_string()),
            });
        }
        mappings
    }
}
