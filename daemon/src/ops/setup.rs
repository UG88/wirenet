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
        let _ = Command::new("systemctl").args(["stop", "rinetd", "haproxy", "wirenet-gateway"]).output();
        let _ = Command::new("systemctl").args(["disable", "rinetd", "haproxy"]).output();

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

        // 4. Write /etc/wireguard/wg0.conf (Preserving any existing [Peer] blocks)
        println!("[4/5] Writing /etc/wireguard/wg0.conf configuration...");
        let default_iface = Self::get_default_iface();
        let existing_peers = Self::extract_existing_peers();
        let wg_conf = format!(
            "[Interface]\n\
            Address = 10.200.0.1/24\n\
            ListenPort = 51820\n\
            PrivateKey = {}\n\
            SaveConfig = false\n\n\
            PostUp = iptables -I FORWARD 1 -j ACCEPT; iptables -I INPUT 1 -i wg0 -j ACCEPT; iptables -t nat -I PREROUTING 1 -p tcp -m multiport --dports {}:{} -j DNAT --to-destination 10.200.0.2; iptables -t nat -I PREROUTING 1 -p udp -m multiport --dports {}:{} -j DNAT --to-destination 10.200.0.2; iptables -t nat -A POSTROUTING -o {} -j MASQUERADE\n\
            PostDown = iptables -D FORWARD -j ACCEPT 2>/dev/null; iptables -D INPUT -i wg0 -j ACCEPT 2>/dev/null; iptables -t nat -D PREROUTING -p tcp -m multiport --dports {}:{} -j DNAT --to-destination 10.200.0.2 2>/dev/null; iptables -t nat -D PREROUTING -p udp -m multiport --dports {}:{} -j DNAT --to-destination 10.200.0.2 2>/dev/null; iptables -t nat -D POSTROUTING -o {} -j MASQUERADE 2>/dev/null\n\n\
            {}",
            priv_key, ports_start, ports_end, ports_start, ports_end, default_iface, ports_start, ports_end, ports_start, ports_end, default_iface, existing_peers
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
        if existing_peers.is_empty() {
            println!(" Next Step: On your Node VPS, run:");
            println!("   wirenet setup node --gateway <YOUR_GATEWAY_PUBLIC_IP> --gateway-key \"{}\"", pub_key);
        } else {
            println!(" [✓] Existing authorized Node peers were preserved!");
        }
        println!("==========================================================");

        Ok(())
    }

    /// Automatically applies Real IP Routing in place while keeping all existing keys and peers
    pub fn apply_real_ip_routing() -> Result<()> {
        println!("==========================================================");
        println!(" ⚡ WireNet In-Place Real IP Routing Auto-Applier");
        println!("==========================================================");

        let is_gateway = fs::read_to_string("/etc/wireguard/wg0.conf")
            .map(|c| c.contains("10.200.0.1/24"))
            .unwrap_or(false)
            || std::path::Path::new("/etc/wireguard/gateway_private.key").exists();

        if is_gateway {
            println!("[+] Detected Role: GATEWAY VPS (Hub)");
            Self::setup_gateway(25565, 25700)?;
        } else {
            println!("[+] Detected Role: NODE VPS (Spoke)");
            let mut gw_key = String::new();
            let mut gw_endpoint = String::new();

            if let Ok(c) = fs::read_to_string("/etc/wireguard/wg0.conf") {
                for line in c.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("PublicKey") {
                        if let Some((_, val)) = trimmed.split_once('=') {
                            gw_key = val.trim().to_string();
                        }
                    } else if trimmed.starts_with("Endpoint") {
                        if let Some((_, val)) = trimmed.split_once('=') {
                            if let Some(ip) = val.trim().split(':').next() {
                                gw_endpoint = ip.trim().to_string();
                            }
                        }
                    }
                }
            }

            // Auto-repair base64 padding if trailing '=' was stripped
            if gw_key.len() == 43 {
                gw_key.push('=');
            }

            if gw_key.is_empty() || gw_endpoint.is_empty() {
                println!(" [!] Existing Gateway peer not found in /etc/wireguard/wg0.conf.");
                println!(" Please run: wirenet setup node --gateway <IP> --gateway-key <KEY>");
                return Ok(());
            }

            Self::setup_node(&gw_endpoint, &gw_key)?;
        }

        println!("==========================================================");
        println!(" [✓] Real IP Routing Active! Existing Keys & Peers Preserved.");
        println!("==========================================================");
        Ok(())
    }

    fn extract_existing_peers() -> String {
        if let Ok(existing) = fs::read_to_string("/etc/wireguard/wg0.conf") {
            let mut peers_text = String::new();
            let mut in_peer = false;
            for line in existing.lines() {
                if line.trim().starts_with("[Peer]") {
                    in_peer = true;
                }
                if in_peer {
                    peers_text.push_str(line);
                    peers_text.push('\n');
                }
            }
            peers_text
        } else {
            String::new()
        }
    }

    /// Setup Node (Spoke) VPS
    pub fn setup_node(gateway_ip: &str, gateway_pub_key: &str) -> Result<()> {
        println!("==========================================================");
        println!(" 🚀 WireNet Pterodactyl Node VPS (Spoke) 1-Click Setup");
        println!("==========================================================");

        // 1. Enable IP Forwarding, Route Localnet & Loose RP Filter
        println!("[1/5] Enabling Kernel IP Forwarding & Policy Routing...");
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.ip_forward=1"]).output();
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.all.route_localnet=1"]).output();
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.default.route_localnet=1"]).output();
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.all.rp_filter=2"]).output();
        let _ = Command::new("sysctl").args(["-w", "net.ipv4.conf.default.rp_filter=2"]).output();
        let _ = Command::new("systemctl").args(["stop", "rinetd", "haproxy"]).output();
        let _ = Command::new("systemctl").args(["disable", "rinetd", "haproxy"]).output();

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
        let primary_ip = Self::get_primary_ip();

        let mut custom_post_up = String::new();
        let mut custom_post_down = String::new();
        
        let out = Command::new("docker").args(["ps", "--format", "{{.ID}}\t{{.Ports}}"]).output();
        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout);
            for line in s.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() < 2 { continue; }
                let cid = parts[0];
                let ports_str = parts[1];
                let ip_out = Command::new("docker")
                    .args(["inspect", cid, "--format", "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}"])
                    .output();
                if let Ok(io) = ip_out {
                    let cip = String::from_utf8_lossy(&io.stdout).trim().to_string();
                    if !cip.is_empty() {
                        for port_part in ports_str.split(',') {
                            if let Some(arrow) = port_part.find("->") {
                                let host_p = port_part[..arrow].split(':').last().unwrap_or("0").trim();
                                let cont_p = port_part[arrow + 2..].split('/').next().unwrap_or("0").trim();
                                if let (Ok(hp), Ok(cp)) = (host_p.parse::<u16>(), cont_p.parse::<u16>()) {
                                    if hp >= 1024 {
                                        custom_post_up.push_str(&format!("PostUp = iptables -t nat -I PREROUTING 1 -i wg0 -p tcp --dport {} -j DNAT --to-destination {}:{}\n", hp, cip, cp));
                                        custom_post_up.push_str(&format!("PostUp = iptables -t nat -I PREROUTING 1 -i wg0 -p udp --dport {} -j DNAT --to-destination {}:{}\n", hp, cip, cp));
                                        custom_post_up.push_str(&format!("PostUp = iptables -t nat -I PREROUTING 1 -d 10.200.0.2 -p tcp --dport {} -j DNAT --to-destination {}:{}\n", hp, cip, cp));
                                        custom_post_up.push_str(&format!("PostUp = iptables -t nat -I PREROUTING 1 -d 10.200.0.2 -p udp --dport {} -j DNAT --to-destination {}:{}\n", hp, cip, cp));
                                        
                                        custom_post_down.push_str(&format!("PostDown = iptables -t nat -D PREROUTING -i wg0 -p tcp --dport {} -j DNAT --to-destination {}:{}\n", hp, cip, cp));
                                        custom_post_down.push_str(&format!("PostDown = iptables -t nat -D PREROUTING -i wg0 -p udp --dport {} -j DNAT --to-destination {}:{}\n", hp, cip, cp));
                                        custom_post_down.push_str(&format!("PostDown = iptables -t nat -D PREROUTING -d 10.200.0.2 -p tcp --dport {} -j DNAT --to-destination {}:{}\n", hp, cip, cp));
                                        custom_post_down.push_str(&format!("PostDown = iptables -t nat -D PREROUTING -d 10.200.0.2 -p udp --dport {} -j DNAT --to-destination {}:{}\n", hp, cip, cp));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let wg_conf = format!(
            "[Interface]\n\
            Address = 10.200.0.2/24\n\
            PrivateKey = {}\n\
            Table = off\n\n\
            PostUp = ip rule add fwmark 0x1 table 100\n\
            PostUp = ip route add default via 10.200.0.1 dev wg0 table 100\n\
            PostUp = iptables -t mangle -I PREROUTING 1 -i wg0 -m conntrack --ctstate NEW -j CONNMARK --set-mark 0x1\n\
            PostUp = iptables -t mangle -I PREROUTING 1 -j CONNMARK --restore-mark\n\
            PostUp = iptables -t mangle -I OUTPUT 1 -j CONNMARK --restore-mark\n\
            PostUp = iptables -I INPUT 1 -i wg0 -j ACCEPT\n\
            PostUp = iptables -I INPUT 1 -i lo -j ACCEPT\n\
            PostUp = iptables -I FORWARD 1 -i wg0 -j ACCEPT\n\
            PostUp = iptables -I FORWARD 1 -o wg0 -j ACCEPT\n\
            PostUp = iptables -I DOCKER-USER 1 -j ACCEPT\n\
            PostUp = iptables -t nat -I PREROUTING 1 -i wg0 -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination {}\n\
            PostUp = iptables -t nat -I PREROUTING 1 -i wg0 -p udp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination {}\n\
            PostUp = iptables -t nat -I PREROUTING 1 -d 10.200.0.2 -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination {}\n\
            PostUp = iptables -t nat -I PREROUTING 1 -d 10.200.0.2 -p udp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination {}\n\
            {}\
            PostDown = ip rule del fwmark 0x1 table 100\n\
            PostDown = ip route del default via 10.200.0.1 dev wg0 table 100\n\
            PostDown = iptables -t mangle -D PREROUTING -i wg0 -m conntrack --ctstate NEW -j CONNMARK --set-mark 0x1\n\
            PostDown = iptables -t mangle -D PREROUTING -j CONNMARK --restore-mark\n\
            PostDown = iptables -t mangle -D OUTPUT -j CONNMARK --restore-mark\n\
            PostDown = iptables -t nat -D PREROUTING -i wg0 -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination {}\n\
            PostDown = iptables -t nat -D PREROUTING -i wg0 -p udp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination {}\n\
            PostDown = iptables -t nat -D PREROUTING -d 10.200.0.2 -p tcp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination {}\n\
            PostDown = iptables -t nat -D PREROUTING -d 10.200.0.2 -p udp -m multiport --dports 25565:25700,30000:40000 -j DNAT --to-destination {}\n\
            {}\n\
            [Peer]\n\
            PublicKey = {}\n\
            Endpoint = {}:51820\n\
            AllowedIPs = 0.0.0.0/0\n\
            PersistentKeepalive = 15\n",
            priv_key, primary_ip, primary_ip, primary_ip, primary_ip, custom_post_up, primary_ip, primary_ip, primary_ip, primary_ip, custom_post_down, gateway_pub_key, gateway_ip
        );
        fs::write("/etc/wireguard/wg0.conf", wg_conf)?;

        // 5. Activate Interface & Systemd Service
        println!("[5/5] Activating WireGuard wg0 interface...");
        let _ = Command::new("systemctl").args(["stop", "wg-quick@wg0"]).output();
        let _ = Command::new("ip").args(["link", "del", "dev", "wg0"]).output();
        let up = Command::new("wg-quick").args(["up", "wg0"]).output();
        if let Ok(ref u) = up {
            if !u.status.success() {
                println!("  [!] Notice: {}", String::from_utf8_lossy(&u.stderr).trim());
            }
        }
        let _ = Command::new("systemctl").args(["enable", "--now", "wg-quick@wg0"]).output();

        // Create wirenet-node.service
        Self::install_node_systemd(gateway_ip)?;

        println!("\n==========================================================");
        println!(" [✓] Node VPS Installation Succeeded!");
        println!("==========================================================");
        println!(" Node Virtual IP : 10.200.0.2");
        println!(" Node Public Key : {}", pub_key);
        println!("==========================================================");
        println!(" Crucial Final Step: On your Gateway VPS, run:");
        println!("   wirenet peer add \"{}\"", pub_key);
        println!("==========================================================");

        Ok(())
    }

    /// Add and permanently persist a Node peer on the Gateway
    pub fn add_node_peer(node_pub_key: &str, node_virtual_ip: &str) -> Result<()> {
        let trimmed_key = node_pub_key.trim();
        println!("==========================================================");
        println!(" 🔑 WireNet Gateway Peer Authorizer");
        println!("==========================================================");
        println!("[+] Registering Peer Public Key: {}", trimmed_key);
        println!("[+] Assigned Virtual IP        : {}", node_virtual_ip);

        // 1. Add to active runtime WireGuard interface
        let _ = Command::new("wg").args(["set", "wg0", "peer", trimmed_key, "allowed-ips", &format!("{}/32", node_virtual_ip)]).output();

        // 2. Persist to /etc/wireguard/wg0.conf if not already present
        let mut conf_content = fs::read_to_string("/etc/wireguard/wg0.conf").unwrap_or_default();
        if !conf_content.contains(trimmed_key) {
            conf_content.push_str(&format!(
                "\n[Peer]\nPublicKey = {}\nAllowedIPs = {}/32\n",
                trimmed_key, node_virtual_ip
            ));
            fs::write("/etc/wireguard/wg0.conf", conf_content)?;
            println!("  [✓] Peer permanently appended to /etc/wireguard/wg0.conf");
        } else {
            println!("  [✓] Peer was already registered in /etc/wireguard/wg0.conf");
        }

        println!("==========================================================");
        println!(" [✓] Node Peer Authorized & Persisted Successfully!");
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

    fn get_primary_ip() -> String {
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
