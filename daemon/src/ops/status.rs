use anyhow::Result;
use std::process::Command;

pub struct StatusManager;

impl StatusManager {
    pub fn show_status() -> Result<()> {
        println!("==========================================================");
        println!(" 🌐 WireNet Tunnel & System Telemetry Status");
        println!("==========================================================");

        // 1. Host Info
        let default_ip = Self::get_public_ip();
        println!(" Host Public IP  : {}", default_ip);

        // 2. WireGuard Info
        let wg_out = Command::new("wg").args(["show"]).output();
        if let Ok(o) = wg_out {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout);
                println!("\n--- [ WireGuard Kernel Status ] ---");
                for line in s.lines() {
                    println!(" {}", line);
                }
            } else {
                println!(" WireGuard Status: INACTIVE (Interface wg0 is down)");
            }
        }

        // 3. Service Status
        println!("\n--- [ Background Daemons ] ---");
        let services = ["wirenet-gateway.service", "wirenet-node.service", "wg-quick@wg0.service"];
        for s in services {
            let active = Command::new("systemctl").args(["is-active", s]).output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "inactive".to_string());
            println!(" {:<24}: {}", s, active.to_uppercase());
        }

        println!("==========================================================");
        Ok(())
    }

    fn get_public_ip() -> String {
        let out = Command::new("ip").args(["route", "get", "1.1.1.1"]).output();
        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout);
            let parts: Vec<&str> = s.split_whitespace().collect();
            if let Some(idx) = parts.iter().position(|&r| r == "src") {
                if let Some(ip) = parts.get(idx + 1) {
                    return ip.to_string();
                }
            }
        }
        "127.0.0.1".to_string()
    }
}
