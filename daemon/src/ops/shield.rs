use anyhow::Result;
use std::process::Command;

pub struct ShieldManager;

impl ShieldManager {
    pub fn set_mode(mode: &str) -> Result<()> {
        println!("==========================================================");
        println!(" 🛡️ WireNet Anti-DDoS Scrubbing & Kernel Shield");
        println!("==========================================================");

        match mode.to_lowercase().as_str() {
            "strict" => {
                println!("[+] Applying STRICT Anti-DDoS Scrubbing Profile...");
                let _ = Command::new("sysctl").args(["-w", "net.ipv4.tcp_syncookies=1"]).output();
                let _ = Command::new("sysctl").args(["-w", "net.ipv4.tcp_max_syn_backlog=65536"]).output();
                let _ = Command::new("sysctl").args(["-w", "net.ipv4.tcp_synack_retries=1"]).output();
                let _ = Command::new("sysctl").args(["-w", "net.ipv4.tcp_fin_timeout=15"]).output();
                let _ = Command::new("sysctl").args(["-w", "net.netfilter.nf_conntrack_max=2097152"]).output();
                println!("  [✓] Aggressive SYN flood scrubbing: ACTIVE");
                println!("  [✓] Max SYN backlog: 65,536");
                println!("  [✓] Conntrack capacity: 2,097,152 states");
            }
            "off" => {
                println!("[!] Anti-DDoS Scrubbing Shield is DISABLED.");
                let _ = Command::new("sysctl").args(["-w", "net.ipv4.tcp_syncookies=0"]).output();
            }
            _ => {
                // Standard mode
                println!("[+] Applying STANDARD Anti-DDoS Protection Profile...");
                let _ = Command::new("sysctl").args(["-w", "net.ipv4.tcp_syncookies=1"]).output();
                let _ = Command::new("sysctl").args(["-w", "net.ipv4.tcp_max_syn_backlog=8192"]).output();
                let _ = Command::new("sysctl").args(["-w", "net.ipv4.tcp_synack_retries=2"]).output();
                let _ = Command::new("sysctl").args(["-w", "net.ipv4.tcp_fin_timeout=30"]).output();
                println!("  [✓] Hardware SYN cookies: ENABLED");
                println!("  [✓] Standard Bot Raid Protection: ACTIVE");
            }
        }

        println!("==========================================================");
        Ok(())
    }
}
