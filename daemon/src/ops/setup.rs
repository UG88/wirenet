use anyhow::{Context, Result};
use std::fs;
use std::process::Command;

pub struct SetupManager;

impl SetupManager {
    /// Setup Gateway (Hub) VPS
    pub fn setup_gateway(ports_start: u16, ports_end: u16) -> Result<()> {
        println!("==========================================================");
        println!(" 🌐 WireNet Gateway VPS (Hub) 1-Click Setup");
        println!("==========================================================");

        // 1. Enable IP Forwarding
        println!("[1/5] Enabling Kernel IP Forwarding...");
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.ip_forward=1"]).output();
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.all.forwarding=1"]).output();

        // 2. Ensure WireGuard is installed
        println!("[2/5] Checking WireGuard installation...");
        if !Command::new("which").arg("wg").output().map(|o| o.status.success()).unwrap_or(false) {
            println!("  [+] Installing WireGuard package...");
            let _ = Command::new("apt-get").args(["update", "-qq"]).output();
            let _ = Command::new("apt-get").args(["install", "-y", "-qq", "wireguard", "wireguard-tools", "iptables"]).output();
        }

        // 3. Generate Keys if missing
        println!("[3/5] Generating Cryptographic WireGuard Keys...");
        let _ = fs::create_dir_all("/etc/wireguard");
        
        let priv_key = if let Ok(k) = fs::read_to_string("/etc/wireguard/gateway_private.key") {
            k.trim().to_string()
        } else {
            let out = Command::new("wg").arg("genkey").output().context("Failed to run wg genkey")?;
            let k = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let _ = fs::write("/etc/wireguard/gateway_private.key", &k);
            k
        };

        let pub_key = match Self::derive_public_key(&priv_key) {
            Ok(pk) => {
                let _ = fs::write("/etc/wireguard/gateway_public.key", &pk);
                pk
            }
            Err(e) => {
                println!("  [!] Warning: Failed to derive public key: {:?}", e);
                "UNKNOWN_KEY".to_string()
            }
        };

        // 4. Write /etc/wireguard/wg0.conf
        println!("[4/5] Writing /etc/wireguard/wg0.conf configuration...");
        let default_iface = Self::get_default_iface();
        let wg_conf = format!(
            "[Interface]\n\
            Address = 10.200.0.1/24\n\
            ListenPort = 51820\n\
            PrivateKey = {}\n\
            SaveConfig = false\n\n\
            PostUp = iptables -I FORWARD 1 -i wg0 -j ACCEPT; iptables -I FORWARD 1 -o wg0 -j ACCEPT; iptables -t nat -A PREROUTING -p tcp -m multiport --dports {}:{} -j DNAT --to-destination 10.200.0.2; iptables -t nat -A PREROUTING -p udp -m multiport --dports {}:{} -j DNAT --to-destination 10.200.0.2; iptables -t nat -A POSTROUTING -o {} -j MASQUERADE\n\
            PostDown = iptables -D FORWARD -i wg0 -j ACCEPT 2>/dev/null; iptables -D FORWARD -o wg0 -j ACCEPT 2>/dev/null; iptables -t nat -D PREROUTING -p tcp -m multiport --dports {}:{} -j DNAT --to-destination 10.200.0.2 2>/dev/null; iptables -t nat -D PREROUTING -p udp -m multiport --dports {}:{} -j DNAT --to-destination 10.200.0.2 2>/dev/null; iptables -t nat -D POSTROUTING -o {} -j MASQUERADE 2>/dev/null\n",
            priv_key, ports_start, ports_end, ports_start, ports_end, default_iface, ports_start, ports_end, ports_start, ports_end, default_iface
        );
        fs::write("/etc/wireguard/wg0.conf", wg_conf)?;

        // 5. Activate Interface & Systemd Service
        println!("[5/5] Activating WireGuard wg0 interface...");
        let _ = Command::new("systemctl").args(["enable", "--now", "wg-quick@wg0"]).output();
        let _ = Command::new("systemctl").args(["restart", "wg-quick@wg0"]).output();

        // Create wirenet-gateway.service
        Self::install_gateway_systemd(ports_start, ports_end)?;

        println!("\n==========================================================");
        println!(" [✓] Gateway VPS Installation Succeeded!");
        println!("==========================================================");
        println!(" Gateway Virtual IP : 10.200.0.1");
        println!(" Gateway Public Key : {}", pub_key);
        println!(" Game Port Range    : {}-{}", ports_start, ports_end);
        println!("==========================================================");
        println!(" Next Step: On your Node VPS, run:");
        println!("   wirenet setup node --gateway <YOUR_GATEWAY_PUBLIC_IP> --gateway-key \"{}\"", pub_key);
        println!("==========================================================");

        Ok(())
    }

    /// Setup Node (Spoke) VPS
    pub fn setup_node(gateway_ip: &str, gateway_pub_key: &str) -> Result<()> {
        println!("==========================================================");
        println!(" 🚀 WireNet Pterodactyl Node VPS (Spoke) 1-Click Setup");
        println!("==========================================================");

        // 1. Enable IP Forwarding & Loose RP Filter
        println!("[1/5] Enabling Kernel IP Forwarding & Policy Routing...");
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.ip_forward=1"]).output();
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.all.rp_filter=2"]).output();
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.default.rp_filter=2"]).output();

        // 2. Ensure WireGuard is installed
        println!("[2/5] Checking WireGuard installation...");
        if !Command::new("which").arg("wg").output().map(|o| o.status.success()).unwrap_or(false) {
            println!("  [+] Installing WireGuard package...");
            let _ = Command::new("apt-get").args(["update", "-qq"]).output();
            let _ = Command::new("apt-get").args(["install", "-y", "-qq", "wireguard", "wireguard-tools", "iptables"]).output();
        }

        // 3. Generate Keys
        println!("[3/5] Generating Node Cryptographic Keys...");
        let _ = fs::create_dir_all("/etc/wireguard");
        
        let priv_key = if let Ok(k) = fs::read_to_string("/etc/wireguard/node_private.key") {
            k.trim().to_string()
        } else {
            let out = Command::new("wg").arg("genkey").output().context("Failed to run wg genkey")?;
            let k = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let _ = fs::write("/etc/wireguard/node_private.key", &k);
            k
        };

        let pub_key = match Self::derive_public_key(&priv_key) {
            Ok(pk) => {
                let _ = fs::write("/etc/wireguard/node_public.key", &pk);
                pk
            }
            Err(e) => {
                println!("  [!] Warning: Failed to derive public key: {:?}", e);
                "UNKNOWN_KEY".to_string()
            }
        };

        // 4. Write /etc/wireguard/wg0.conf with Policy Routing (Table = off & AllowedIPs = 0.0.0.0/0)
        println!("[4/5] Configuring WireGuard Node Interface & Symmetric Return Rules...");
        let default_iface = Self::get_default_iface();
        let wg_conf = format!(
            "[Interface]\n\
            Address = 10.200.0.2/24\n\
            PrivateKey = {}\n\
            Table = off\n\n\
            PostUp = ip rule add fwmark 0x1 table 100 2>/dev/null || true; ip route add default via 10.200.0.1 dev wg0 table 100 2>/dev/null || true; iptables -t mangle -A PREROUTING -i wg0 -m conntrack --ctstate NEW -j CONNMARK --set-mark 0x1; iptables -t mangle -A PREROUTING -j CONNMARK --restore-mark; iptables -t mangle -A OUTPUT -j CONNMARK --restore-mark; iptables -I INPUT 1 -i wg0 -j ACCEPT; iptables -I INPUT 1 -i lo -j ACCEPT; iptables -I FORWARD 1 -i wg0 -j ACCEPT; iptables -I FORWARD 1 -o wg0 -j ACCEPT; iptables -A INPUT -i {} -p tcp -m multiport --dports 25565:25700,30000:40000 -j DROP; iptables -A INPUT -i {} -p udp -m multiport --dports 25565:25700,30000:40000 -j DROP\n\
            PostDown = ip rule del fwmark 0x1 table 100 2>/dev/null || true; ip route del default via 10.200.0.1 dev wg0 table 100 2>/dev/null || true; iptables -t mangle -D PREROUTING -i wg0 -m conntrack --ctstate NEW -j CONNMARK --set-mark 0x1 2>/dev/null || true; iptables -t mangle -D PREROUTING -j CONNMARK --restore-mark 2>/dev/null || true; iptables -t mangle -D OUTPUT -j CONNMARK --restore-mark 2>/dev/null || true; iptables -D INPUT -i {} -p tcp -m multiport --dports 25565:25700,30000:40000 -j DROP 2>/dev/null || true; iptables -D INPUT -i {} -p udp -m multiport --dports 25565:25700,30000:40000 -j DROP 2>/dev/null || true\n\n\
            [Peer]\n\
            PublicKey = {}\n\
            Endpoint = {}:51820\n\
            AllowedIPs = 0.0.0.0/0\n\
            PersistentKeepalive = 15\n",
            priv_key, default_iface, default_iface, default_iface, default_iface, gateway_pub_key, gateway_ip
        );
        fs::write("/etc/wireguard/wg0.conf", wg_conf)?;

        // 5. Activate Interface & Systemd Service
        println!("[5/5] Activating WireGuard wg0 interface...");
        let _ = Command::new("systemctl").args(["enable", "--now", "wg-quick@wg0"]).output();
        let _ = Command::new("systemctl").args(["restart", "wg-quick@wg0"]).output();

        // Create wirenet-node.service
        Self::install_node_systemd(gateway_ip)?;

        println!("\n==========================================================");
        println!(" [✓] Node VPS Installation Succeeded!");
        println!("==========================================================");
        println!(" Node Virtual IP : 10.200.0.2");
        println!(" Node Public Key : {}", pub_key);
        println!("==========================================================");
        println!(" Crucial Final Step: On your Gateway VPS, run:");
        println!("   wg set wg0 peer \"{}\" allowed-ips 10.200.0.2/32", pub_key);
        println!("==========================================================");

        Ok(())
    }

    fn get_default_iface() -> String {
        let out = Command::new("ip").args(["route", "show", "default"]).output();
        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout);
            let parts: Vec<&str> = s.split_whitespace().collect();
            if let Some(idx) = parts.iter().position(|&r| r == "dev") {
                if let Some(iface) = parts.get(idx + 1) {
                    return iface.to_string();
                }
            }
        }
        "eth0".to_string()
    }

    fn install_gateway_systemd(start_port: u16, end_port: u16) -> Result<()> {
        let content = format!(
            "[Unit]\n\
            Description=WireNet Rust Gateway Ingress & Anti-DDoS Daemon\n\
            After=network.target wg-quick@wg0.service\n\
            Wants=wg-quick@wg0.service\n\n\
            [Service]\n\
            Type=simple\n\
            ExecStart=/usr/local/bin/wirenet gateway run --start-port {} --end-port {}\n\
            Restart=always\n\
            RestartSec=3\n\
            LimitNOFILE=65535\n\n\
            [Install]\n\
            WantedBy=multi-user.target\n",
            start_port, end_port
        );
        let _ = fs::write("/etc/systemd/system/wirenet-gateway.service", content);
        let _ = Command::new("systemctl").args(["daemon-reload"]).output();
        let _ = Command::new("systemctl").args(["enable", "--now", "wirenet-gateway.service"]).output();
        let _ = Command::new("systemctl").args(["restart", "wirenet-gateway.service"]).output();
        Ok(())
    }

    fn install_node_systemd(_gateway_ip: &str) -> Result<()> {
        let content = format!(
            "[Unit]\n\
            Description=WireNet Rust Node Agent & Docker Bridge Daemon\n\
            After=network.target wg-quick@wg0.service docker.service\n\
            Wants=wg-quick@wg0.service docker.service\n\n\
            [Service]\n\
            Type=simple\n\
            ExecStart=/usr/local/bin/wirenet node run --gateway 10.200.0.1:9000\n\
            Restart=always\n\
            RestartSec=3\n\
            LimitNOFILE=65535\n\n\
            [Install]\n\
            WantedBy=multi-user.target\n"
        );
        let _ = fs::write("/etc/systemd/system/wirenet-node.service", content);
        let _ = Command::new("systemctl").args(["daemon-reload"]).output();
        let _ = Command::new("systemctl").args(["enable", "--now", "wirenet-node.service"]).output();
        let _ = Command::new("systemctl").args(["restart", "wirenet-node.service"]).output();
        Ok(())
    }

    fn derive_public_key(priv_key: &str) -> Result<String> {
        use std::io::Write;
        let mut child = Command::new("wg")
            .arg("pubkey")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .context("Failed to spawn wg pubkey")?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = writeln!(stdin, "{}", priv_key.trim());
            drop(stdin); // Explicitly close stdin to send EOF
        }

        let out = child.wait_with_output().context("Failed to wait for wg pubkey output")?;
        let pub_key = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if pub_key.is_empty() {
            return Err(anyhow::anyhow!("wg pubkey returned an empty string"));
        }
        Ok(pub_key)
    }
}
