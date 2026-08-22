use anyhow::Result;
use std::process::Command;

pub struct UninstallManager;

impl UninstallManager {
    pub fn deep_uninstall() -> Result<()> {
        println!("==========================================================");
        println!(" 🗑️ WireNet 100% Deep Cleaner & Uninstaller");
        println!("==========================================================");

        println!("[1/5] Stopping and disabling background services...");
        let _ = Command::new("systemctl").args(["stop", "wirenet-gateway.service", "wirenet-node.service", "wirenet-watcher.service"]).output();
        let _ = Command::new("systemctl").args(["disable", "wirenet-gateway.service", "wirenet-node.service", "wirenet-watcher.service"]).output();

        println!("[2/5] Tearing down WireGuard interfaces...");
        let _ = Command::new("systemctl").args(["stop", "wg-quick@wg0"]).output();
        let _ = Command::new("systemctl").args(["disable", "wg-quick@wg0"]).output();
        let _ = Command::new("ip").args(["link", "del", "dev", "wg0"]).output();

        println!("[3/5] Removing systemd service units...");
        let files = [
            "/etc/systemd/system/wirenet-gateway.service",
            "/etc/systemd/system/wirenet-node.service",
            "/etc/systemd/system/wirenet-watcher.service",
        ];
        for f in files {
            let _ = std::fs::remove_file(f);
        }
        let _ = Command::new("systemctl").args(["daemon-reload"]).output();

        println!("[4/5] Removing WireNet configurations & keys...");
        let _ = std::fs::remove_dir_all("/opt/wirenet");
        let _ = std::fs::remove_file("/etc/wireguard/wg0.conf");
        let _ = std::fs::remove_file("/etc/wireguard/gateway_private.key");
        let _ = std::fs::remove_file("/etc/wireguard/gateway_public.key");
        let _ = std::fs::remove_file("/etc/wireguard/node_private.key");
        let _ = std::fs::remove_file("/etc/wireguard/node_public.key");

        println!("[5/5] Removing binary /usr/local/bin/wirenet...");
        let _ = std::fs::remove_file("/usr/local/bin/wirenet");

        println!("==========================================================");
        println!(" [✓] WireNet Completely Removed from this Server!");
        println!("==========================================================");

        Ok(())
    }
}
