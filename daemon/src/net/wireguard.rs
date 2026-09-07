use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WireGuardInterface {
    pub name: String,
    pub private_key: String,
    pub listen_port: u16,
    pub address: String,
    pub table: Option<String>,
    pub peers: Vec<WireGuardPeer>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WireGuardPeer {
    pub public_key: String,
    pub endpoint: Option<String>,
    pub allowed_ips: Vec<String>,
    pub persistent_keepalive: Option<u16>,
}

impl WireGuardInterface {
    pub fn new(name: &str, private_key: &str, listen_port: u16, address: &str) -> Self {
        Self {
            name: name.to_string(),
            private_key: private_key.trim().to_string(),
            listen_port,
            address: address.trim().to_string(),
            table: None,
            peers: Vec::new(),
        }
    }

    pub fn with_table(mut self, table: &str) -> Self {
        self.table = Some(table.to_string());
        self
    }

    pub fn add_peer(&mut self, peer: WireGuardPeer) {
        self.peers.push(peer);
    }

    pub fn render_config(&self) -> String {
        let mut conf = String::new();
        conf.push_str("[Interface]\n");
        conf.push_str(&format!("Address = {}\n", self.address));
        conf.push_str(&format!("ListenPort = {}\n", self.listen_port));
        conf.push_str(&format!("PrivateKey = {}\n", self.private_key));

        if let Some(table) = &self.table {
            conf.push_str(&format!("Table = {}\n", table));
        }

        for peer in &self.peers {
            conf.push_str("\n[Peer]\n");
            conf.push_str(&format!("PublicKey = {}\n", peer.public_key));
            if !peer.allowed_ips.is_empty() {
                conf.push_str(&format!("AllowedIPs = {}\n", peer.allowed_ips.join(", ")));
            }
            if let Some(endpoint) = &peer.endpoint {
                conf.push_str(&format!("Endpoint = {}\n", endpoint));
            }
            if let Some(keepalive) = peer.persistent_keepalive {
                conf.push_str(&format!("PersistentKeepalive = {}\n", keepalive));
            }
        }

        conf
    }

    pub fn write_config(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }

        let content = self.render_config();
        fs::write(path, content)
            .with_context(|| format!("writing wireguard configuration to {}", path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .with_context(|| format!("securing permissions for {}", path.display()))?;
        }

        Ok(())
    }

    pub fn apply(&self, conf_path: &Path) -> Result<()> {
        self.write_config(conf_path)?;

        // Try syncing live interface first (zero downtime)
        let sync_res = Command::new("wg")
            .args(["syncconf", &self.name, &conf_path.to_string_lossy()])
            .output();

        if let Ok(out) = sync_res {
            if out.status.success() {
                return Ok(());
            }
        }

        // Interface may not be up yet; try bringing it up via wg-quick
        let up_res = Command::new("wg-quick")
            .args(["up", &conf_path.to_string_lossy()])
            .output();

        if let Ok(out) = up_res {
            if out.status.success() {
                return Ok(());
            }
        }

        // Direct ip link fallback
        let _ = Command::new("ip")
            .args(["link", "add", "dev", &self.name, "type", "wireguard"])
            .output();
        let _ = Command::new("ip")
            .args(["addr", "add", &self.address, "dev", &self.name])
            .output();
        let setconf = Command::new("wg")
            .args(["setconf", &self.name, &conf_path.to_string_lossy()])
            .output();
        let _ = Command::new("ip")
            .args(["link", "set", "up", "dev", &self.name])
            .output();

        if let Ok(out) = setconf {
            if !out.status.success() {
                bail!(
                    "failed to apply wireguard config: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        }

        Ok(())
    }
}

pub fn generate_keypair() -> Result<(String, String)> {
    let priv_out = Command::new("wg")
        .arg("genkey")
        .output()
        .context("running wg genkey")?;

    if !priv_out.status.success() {
        bail!(
            "wg genkey failed: {}",
            String::from_utf8_lossy(&priv_out.stderr)
        );
    }

    let priv_key = String::from_utf8_lossy(&priv_out.stdout).trim().to_string();

    let mut pub_cmd = Command::new("wg");
    pub_cmd.arg("pubkey");
    #[cfg(windows)]
    let mut child = {
        use std::process::Stdio;
        pub_cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("spawning wg pubkey")?
    };
    #[cfg(not(windows))]
    let mut child = {
        use std::process::Stdio;
        pub_cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("spawning wg pubkey")?
    };

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(priv_key.as_bytes())?;
    }

    let pub_out = child.wait_with_output().context("waiting on wg pubkey")?;
    if !pub_out.status.success() {
        bail!(
            "wg pubkey failed: {}",
            String::from_utf8_lossy(&pub_out.stderr)
        );
    }

    let pub_key = String::from_utf8_lossy(&pub_out.stdout).trim().to_string();
    Ok((priv_key, pub_key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_gateway_config() {
        let mut iface = WireGuardInterface::new(
            "wg0",
            "aGVsbG93b3JsZHByaXZhdGVrZXkxMjM0NTY3ODkwMTI=",
            51820,
            "10.200.0.1/24",
        );

        iface.add_peer(WireGuardPeer {
            public_key: "bm9kZXB1YmxpY2tleTEyMzQ1Njc4OTAxMjM0NTY3ODk=".to_string(),
            endpoint: Some("198.51.100.5:51820".to_string()),
            allowed_ips: vec!["10.200.0.2/32".to_string()],
            persistent_keepalive: None,
        });

        let conf = iface.render_config();
        assert!(conf.contains("Address = 10.200.0.1/24"));
        assert!(conf.contains("ListenPort = 51820"));
        assert!(conf.contains("AllowedIPs = 10.200.0.2/32"));
        assert!(conf.contains("Endpoint = 198.51.100.5:51820"));
        assert!(!conf.contains("Table = off"));
    }

    #[test]
    fn test_render_node_config_with_table_off() {
        let mut iface = WireGuardInterface::new(
            "wg0",
            "bm9kZXByaXZhdGVrZXkxMjM0NTY3ODkwMTIzNDU2Nzg=",
            51820,
            "10.200.0.2/32",
        )
        .with_table("off");

        iface.add_peer(WireGuardPeer {
            public_key: "Z2F0ZXdheXB1YmtleTEyMzQ1Njc4OTAxMjM0NTY3ODk=".to_string(),
            endpoint: Some("198.51.100.1:51820".to_string()),
            allowed_ips: vec!["0.0.0.0/0".to_string()],
            persistent_keepalive: Some(25),
        });

        let conf = iface.render_config();
        assert!(conf.contains("Address = 10.200.0.2/32"));
        assert!(conf.contains("Table = off"));
        assert!(conf.contains("AllowedIPs = 0.0.0.0/0"));
        assert!(conf.contains("PersistentKeepalive = 25"));
    }
}
