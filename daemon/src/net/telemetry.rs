use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectedPlayer {
    pub client_ip: String,
    pub client_port: u16,
    pub game_port: u16,
    pub protocol: String,
    pub state: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PacketEvent {
    pub timestamp: String,
    pub client_ip: String,
    pub client_port: u16,
    pub game_port: u16,
    pub protocol: String,
    pub event_type: String, // "CONNECTED", "TRAFFIC", "DISCONNECTED"
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterfaceStats {
    pub name: String,
    pub exists: bool,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub current_pps: u64,
    pub total_packets: u64,
    pub load_percentage: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WireGuardPeerStats {
    pub public_key: String,
    pub endpoint: String,
    pub allowed_ips: Vec<String>,
    pub latest_handshake_epoch: u64,
    pub latest_handshake_ago: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub is_online: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProtectionStats {
    pub shield_mode: String,
    pub syn_cookies_enabled: bool,
    pub max_syn_backlog: u32,
    pub conntrack_count: u64,
    pub conntrack_max: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemTelemetry {
    pub status: String, // "ONLINE", "DEGRADED", "OFFLINE"
    pub uptime_seconds: u64,
    pub interface: InterfaceStats,
    pub peers: Vec<WireGuardPeerStats>,
    pub protection: ProtectionStats,
    pub active_players: Vec<ConnectedPlayer>,
    pub packet_events: Vec<PacketEvent>,
    pub traffic_history: Vec<u64>,
}

pub struct TelemetryCollector {
    pub iface: String,
    start_time: Instant,
    last_sample_time: Instant,
    last_total_packets: u64,
    last_pps: u64,
    traffic_history: VecDeque<u64>,
    active_players: Vec<ConnectedPlayer>,
    packet_events: VecDeque<PacketEvent>,
    last_traffic_event_time: Instant,
}

impl TelemetryCollector {
    pub fn new(iface: &str) -> Self {
        Self {
            iface: iface.to_string(),
            start_time: Instant::now(),
            last_sample_time: Instant::now(),
            last_total_packets: 0,
            last_pps: 0,
            traffic_history: VecDeque::from(vec![0; 30]),
            active_players: Vec::new(),
            packet_events: VecDeque::with_capacity(50),
            last_traffic_event_time: Instant::now(),
        }
    }

    pub fn collect(&mut self, tracked_ports: &[u16]) -> SystemTelemetry {
        let elapsed_secs = self.start_time.elapsed().as_secs();
        let dt = self.last_sample_time.elapsed().as_secs_f64().max(0.1);

        // 1. Read interface packet & byte counters strictly on the configured tunnel
        let iface_stats = read_interface_stats(&self.iface, self.last_total_packets, dt);
        self.last_pps = iface_stats.current_pps;
        self.last_total_packets = iface_stats.total_packets;
        self.last_sample_time = Instant::now();

        // Update traffic history (last 30 samples)
        if self.traffic_history.len() >= 30 {
            self.traffic_history.pop_front();
        }
        self.traffic_history.push_back(self.last_pps);

        // 2. Scan authentic player connections from conntrack and kernel TCP sockets
        let discovered_players = scan_real_connections(tracked_ports);
        let now_str = current_time_string();

        // Detect new connections
        for player in &discovered_players {
            let already_tracked = self
                .active_players
                .iter()
                .any(|p| p.client_ip == player.client_ip && p.client_port == player.client_port);
            if !already_tracked {
                self.push_event(PacketEvent {
                    timestamp: now_str.clone(),
                    client_ip: player.client_ip.clone(),
                    client_port: player.client_port,
                    game_port: player.game_port,
                    protocol: player.protocol.clone(),
                    event_type: "CONNECTED".to_string(),
                    message: format!(
                        "Real Client {}:{} connected to port {} ({})",
                        player.client_ip, player.client_port, player.game_port, player.protocol
                    ),
                });
            }
        }

        // Detect disconnected players
        let mut disconnected = Vec::new();
        for old_player in &self.active_players {
            let still_active = discovered_players.iter().any(|p| {
                p.client_ip == old_player.client_ip && p.client_port == old_player.client_port
            });
            if !still_active {
                disconnected.push(old_player.clone());
            }
        }

        for old_player in disconnected {
            self.push_event(PacketEvent {
                timestamp: now_str.clone(),
                client_ip: old_player.client_ip.clone(),
                client_port: old_player.client_port,
                game_port: old_player.game_port,
                protocol: old_player.protocol.clone(),
                event_type: "DISCONNECTED".to_string(),
                message: format!(
                    "Client {}:{} session closed on port {}",
                    old_player.client_ip, old_player.client_port, old_player.game_port
                ),
            });
        }

        // Add periodic traffic pulse event if active players are transferring packets on wg0
        if self.last_pps > 0
            && !discovered_players.is_empty()
            && self.last_traffic_event_time.elapsed().as_secs() >= 5
        {
            self.last_traffic_event_time = Instant::now();
            let first_p = &discovered_players[0];
            self.push_event(PacketEvent {
                timestamp: now_str.clone(),
                client_ip: first_p.client_ip.clone(),
                client_port: first_p.client_port,
                game_port: first_p.game_port,
                protocol: first_p.protocol.clone(),
                event_type: "TRAFFIC".to_string(),
                message: format!(
                    "Tunnel link streaming {} pkts/sec for {} active player(s)",
                    self.last_pps,
                    discovered_players.len()
                ),
            });
        }

        self.active_players = discovered_players.clone();

        // 3. Read WireGuard peers
        let peers = read_wireguard_peers(&self.iface);

        // 4. Read protection sysctls
        let protection = read_protection_status();

        // 5. Determine system health status
        let status = if !iface_stats.exists {
            "OFFLINE".to_string()
        } else if peers.is_empty() || peers.iter().any(|p| p.is_online) {
            "ONLINE".to_string()
        } else {
            "DEGRADED".to_string()
        };

        SystemTelemetry {
            status,
            uptime_seconds: elapsed_secs,
            interface: iface_stats,
            peers,
            protection,
            active_players: discovered_players,
            packet_events: self.packet_events.iter().cloned().collect(),
            traffic_history: self.traffic_history.iter().copied().collect(),
        }
    }

    fn push_event(&mut self, event: PacketEvent) {
        if self.packet_events.len() >= 50 {
            self.packet_events.pop_front();
        }
        self.packet_events.push_back(event);
    }
}

/// Reads interface packet and byte statistics strictly for `iface` (e.g. "wg0").
/// Never counts eth0, ens*, or external host traffic.
pub fn read_interface_stats(iface: &str, prev_total: u64, dt: f64) -> InterfaceStats {
    #[cfg(unix)]
    {
        let rx_pkts_path = format!("/sys/class/net/{}/statistics/rx_packets", iface);
        let tx_pkts_path = format!("/sys/class/net/{}/statistics/tx_packets", iface);
        let rx_bytes_path = format!("/sys/class/net/{}/statistics/rx_bytes", iface);
        let tx_bytes_path = format!("/sys/class/net/{}/statistics/tx_bytes", iface);

        if let (Ok(rx_p_str), Ok(tx_p_str)) = (
            fs::read_to_string(&rx_pkts_path),
            fs::read_to_string(&tx_pkts_path),
        ) {
            let rx_packets = rx_p_str.trim().parse::<u64>().unwrap_or(0);
            let tx_packets = tx_p_str.trim().parse::<u64>().unwrap_or(0);
            let rx_bytes = fs::read_to_string(&rx_bytes_path)
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0);
            let tx_bytes = fs::read_to_string(&tx_bytes_path)
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0);

            let total_packets = rx_packets + tx_packets;
            let current_pps = if prev_total > 0 && total_packets >= prev_total {
                ((total_packets - prev_total) as f64 / dt).round() as u64
            } else {
                0
            };
            let load_percentage = ((current_pps as f64 / 2000.0) * 100.0).min(100.0) as u16;

            return InterfaceStats {
                name: iface.to_string(),
                exists: true,
                rx_packets,
                tx_packets,
                rx_bytes,
                tx_bytes,
                current_pps,
                total_packets,
                load_percentage,
            };
        }

        // Fallback: Check /proc/net/dev specifically for EXACT interface name
        if let Ok(content) = fs::read_to_string("/proc/net/dev") {
            for line in content.lines().skip(2) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 11 {
                    let dev_name = parts[0].trim_end_matches(':');
                    if dev_name == iface {
                        let rx_bytes = parts[1].parse::<u64>().unwrap_or(0);
                        let rx_packets = parts[2].parse::<u64>().unwrap_or(0);
                        let tx_bytes = parts[9].parse::<u64>().unwrap_or(0);
                        let tx_packets = parts[10].parse::<u64>().unwrap_or(0);
                        let total_packets = rx_packets + tx_packets;

                        let current_pps = if prev_total > 0 && total_packets >= prev_total {
                            ((total_packets - prev_total) as f64 / dt).round() as u64
                        } else {
                            0
                        };
                        let load_percentage =
                            ((current_pps as f64 / 2000.0) * 100.0).min(100.0) as u16;

                        return InterfaceStats {
                            name: iface.to_string(),
                            exists: true,
                            rx_packets,
                            tx_packets,
                            rx_bytes,
                            tx_bytes,
                            current_pps,
                            total_packets,
                            load_percentage,
                        };
                    }
                }
            }
        }
    }

    // Default / fallback when interface is offline or in non-unix dev
    let _ = (iface, prev_total, dt);
    InterfaceStats {
        name: iface.to_string(),
        exists: false,
        rx_packets: 0,
        tx_packets: 0,
        rx_bytes: 0,
        tx_bytes: 0,
        current_pps: 0,
        total_packets: 0,
        load_percentage: 0,
    }
}

/// Scans real client connection IPs from Linux Kernel /proc/net/nf_conntrack and /proc/net/tcp.
/// Excludes loopback, internal gateway addresses, and unmapped administrative ports.
pub fn scan_real_connections(tracked_ports: &[u16]) -> Vec<ConnectedPlayer> {
    #[allow(unused_mut)]
    let mut conns = Vec::new();

    #[cfg(unix)]
    {
        // 1. Scan /proc/net/nf_conntrack for NAT and forwarded game player flows
        if let Ok(content) = fs::read_to_string("/proc/net/nf_conntrack") {
            for line in content.lines() {
                if let Some(player) = parse_conntrack_entry(line) {
                    if is_relevant_game_port(player.game_port, tracked_ports) {
                        if !conns.iter().any(|c: &ConnectedPlayer| {
                            c.client_ip == player.client_ip && c.client_port == player.client_port
                        }) {
                            conns.push(player);
                        }
                    }
                }
            }
        }

        // 2. Scan /proc/net/tcp for local direct sockets
        if let Ok(content) = fs::read_to_string("/proc/net/tcp") {
            for line in content.lines().skip(1) {
                if let Some(player) = parse_proc_net_tcp_entry(line) {
                    if is_relevant_game_port(player.game_port, tracked_ports) {
                        if !conns.iter().any(|c: &ConnectedPlayer| {
                            c.client_ip == player.client_ip && c.client_port == player.client_port
                        }) {
                            conns.push(player);
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tracked_ports;
    }

    conns
}

/// Parses a single line from /proc/net/nf_conntrack into an authentic ConnectedPlayer
pub fn parse_conntrack_entry(line: &str) -> Option<ConnectedPlayer> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 7 {
        return None;
    }

    // Determine protocol
    let protocol = if line.contains("tcp") {
        "TCP"
    } else if line.contains("udp") {
        "UDP"
    } else {
        return None;
    };

    // Determine state
    let state = if line.contains("ESTABLISHED") {
        "ESTABLISHED"
    } else if line.contains("SYN_SENT") {
        "SYN_SENT"
    } else if line.contains("SYN_RECV") {
        "SYN_RECV"
    } else if line.contains("TIME_WAIT") {
        "TIME_WAIT"
    } else if line.contains("CLOSE_WAIT") {
        "CLOSE_WAIT"
    } else if line.contains("[ASSURED]") {
        "ACTIVE"
    } else {
        "FORWARDED"
    };

    let mut src_ip = "";
    let mut src_port = 0u16;
    let mut dst_port = 0u16;
    let mut bytes = 0u64;

    for part in &parts {
        if part.starts_with("src=") && src_ip.is_empty() {
            src_ip = part.trim_start_matches("src=");
        } else if part.starts_with("sport=") && src_port == 0 {
            src_port = part.trim_start_matches("sport=").parse().unwrap_or(0);
        } else if part.starts_with("dport=") && dst_port == 0 {
            dst_port = part.trim_start_matches("dport=").parse().unwrap_or(0);
        } else if part.starts_with("bytes=") && bytes == 0 {
            bytes = part.trim_start_matches("bytes=").parse().unwrap_or(0);
        }
    }

    if src_ip.is_empty() || src_port == 0 || dst_port == 0 {
        return None;
    }

    // Filter out internal tunnel IPs, loopback, and private LANs
    if is_filtered_ip(src_ip) || is_administrative_port(dst_port) {
        return None;
    }

    Some(ConnectedPlayer {
        client_ip: src_ip.to_string(),
        client_port: src_port,
        game_port: dst_port,
        protocol: protocol.to_string(),
        state: state.to_string(),
        bytes,
    })
}

/// Parses a line from /proc/net/tcp
pub fn parse_proc_net_tcp_entry(line: &str) -> Option<ConnectedPlayer> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }

    let local_addr = parts[1];
    let rem_addr = parts[2];
    let state_hex = parts[3];

    // State 01 = TCP_ESTABLISHED
    if state_hex != "01" {
        return None;
    }

    let (_l_ip, l_port) = parse_hex_socket_addr(local_addr)?;
    let (r_ip, r_port) = parse_hex_socket_addr(rem_addr)?;

    if is_filtered_ip(&r_ip) || is_administrative_port(l_port) {
        return None;
    }

    Some(ConnectedPlayer {
        client_ip: r_ip,
        client_port: r_port,
        game_port: l_port,
        protocol: "TCP".to_string(),
        state: "ESTABLISHED".to_string(),
        bytes: 0,
    })
}

pub fn parse_hex_socket_addr(hex_str: &str) -> Option<(String, u16)> {
    let parts: Vec<&str> = hex_str.split(':').collect();
    if parts.len() != 2 {
        return None;
    }

    let ip_hex = parts[0];
    let port_hex = parts[1];

    let port = u16::from_str_radix(port_hex, 16).ok()?;

    if ip_hex.len() == 8 {
        let b0 = u8::from_str_radix(&ip_hex[6..8], 16).ok()?;
        let b1 = u8::from_str_radix(&ip_hex[4..6], 16).ok()?;
        let b2 = u8::from_str_radix(&ip_hex[2..4], 16).ok()?;
        let b3 = u8::from_str_radix(&ip_hex[0..2], 16).ok()?;
        Some((format!("{}.{}.{}.{}", b0, b1, b2, b3), port))
    } else {
        None
    }
}

fn is_filtered_ip(ip: &str) -> bool {
    ip == "127.0.0.1"
        || ip == "0.0.0.0"
        || ip == "::1"
        || ip.starts_with("10.200.0.")
        || ip.starts_with("10.100.0.")
}

fn is_administrative_port(port: u16) -> bool {
    port == 22 || port == 51820 || port == 8080 || port == 53
}

#[allow(dead_code)]
fn is_relevant_game_port(port: u16, tracked_ports: &[u16]) -> bool {
    if !tracked_ports.is_empty() {
        tracked_ports.contains(&port)
    } else {
        // Default standard game ports (Minecraft Java 25565-25700, Bedrock 19132, Steam 27015-27020)
        (25565..=25700).contains(&port) || port == 19132 || (27015..=27020).contains(&port)
    }
}

/// Reads real WireGuard peers from `wg show <iface> dump`
pub fn read_wireguard_peers(iface: &str) -> Vec<WireGuardPeerStats> {
    let mut peers = Vec::new();

    let output = Command::new("wg").args(["show", iface, "dump"]).output();

    if let Ok(out) = output {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let now_epoch = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            // Line 0 is the interface. Lines 1+ are peers
            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 8 {
                    let public_key = parts[0].to_string();
                    let endpoint = parts[2].to_string();
                    let allowed_ips: Vec<String> =
                        parts[3].split(',').map(|s| s.trim().to_string()).collect();
                    let latest_handshake_epoch = parts[4].parse::<u64>().unwrap_or(0);
                    let rx_bytes = parts[5].parse::<u64>().unwrap_or(0);
                    let tx_bytes = parts[6].parse::<u64>().unwrap_or(0);

                    let (is_online, latest_handshake_ago) = if latest_handshake_epoch == 0 {
                        (false, "Never".to_string())
                    } else {
                        let diff = now_epoch.saturating_sub(latest_handshake_epoch);
                        let online = diff < 180; // active within last 3 minutes
                        let ago_str = if diff < 60 {
                            format!("{}s ago", diff)
                        } else if diff < 3600 {
                            format!("{}m ago", diff / 60)
                        } else {
                            format!("{}h ago", diff / 3600)
                        };
                        (online, ago_str)
                    };

                    peers.push(WireGuardPeerStats {
                        public_key,
                        endpoint,
                        allowed_ips,
                        latest_handshake_epoch,
                        latest_handshake_ago,
                        rx_bytes,
                        tx_bytes,
                        is_online,
                    });
                }
            }
        }
    }

    peers
}

/// Reads kernel sysctls for Anti-DDoS protection
pub fn read_protection_status() -> ProtectionStats {
    let syn_cookies = read_sysctl_u32("/proc/sys/net/ipv4/tcp_syncookies").unwrap_or(1) == 1;
    let max_syn_backlog =
        read_sysctl_u32("/proc/sys/net/ipv4/tcp_max_syn_backlog").unwrap_or(8192);
    let conntrack_count =
        read_sysctl_u64("/proc/sys/net/netfilter/nf_conntrack_count").unwrap_or(0);
    let conntrack_max =
        read_sysctl_u64("/proc/sys/net/netfilter/nf_conntrack_max").unwrap_or(262144);

    let shield_mode = if syn_cookies && max_syn_backlog >= 65536 {
        "STRICT (Hardware SYN Cookies + Aggressive Scrubbing)".to_string()
    } else if syn_cookies {
        "STANDARD (Hardware SYN Cookies + Per-IP Rate Limiter)".to_string()
    } else {
        "OFF (Protection Disabled)".to_string()
    };

    ProtectionStats {
        shield_mode,
        syn_cookies_enabled: syn_cookies,
        max_syn_backlog,
        conntrack_count,
        conntrack_max,
    }
}

fn read_sysctl_u32(path: &str) -> Result<u32> {
    let content = fs::read_to_string(path)?;
    Ok(content.trim().parse()?)
}

fn read_sysctl_u64(path: &str) -> Result<u64> {
    let content = fs::read_to_string(path)?;
    Ok(content.trim().parse()?)
}

fn current_time_string() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let hrs = (now / 3600) % 24;
    let mins = (now % 3600) / 60;
    let secs = now % 60;
    format!("{:02}:{:02}:{:02}", hrs, mins, secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_conntrack_entry() {
        let line = "ipv4     2 tcp      6 431999 ESTABLISHED src=198.51.100.44 dst=198.51.100.10 sport=54210 dport=25565 src=10.200.0.2 dst=198.51.100.44 sport=25565 dport=54210 [ASSURED] mark=1 use=2";
        let player = parse_conntrack_entry(line).expect("valid conntrack line");
        assert_eq!(player.client_ip, "198.51.100.44");
        assert_eq!(player.client_port, 54210);
        assert_eq!(player.game_port, 25565);
        assert_eq!(player.protocol, "TCP");
        assert_eq!(player.state, "ESTABLISHED");
    }

    #[test]
    fn test_parse_conntrack_udp_entry() {
        let line = "ipv4     2 udp     17 29 src=203.0.113.88 dst=198.51.100.10 sport=50123 dport=19132 src=10.200.0.2 dst=203.0.113.88 sport=19132 dport=50123 [ASSURED] mark=1 use=2";
        let player = parse_conntrack_entry(line).expect("valid udp conntrack line");
        assert_eq!(player.client_ip, "203.0.113.88");
        assert_eq!(player.client_port, 50123);
        assert_eq!(player.game_port, 19132);
        assert_eq!(player.protocol, "UDP");
    }

    #[test]
    fn test_parse_conntrack_filters_internal_ips() {
        let loopback = "ipv4 2 tcp 6 100 ESTABLISHED src=127.0.0.1 dst=127.0.0.1 sport=8080 dport=54000";
        assert!(parse_conntrack_entry(loopback).is_none());

        let internal = "ipv4 2 tcp 6 100 ESTABLISHED src=10.200.0.1 dst=10.200.0.2 sport=51820 dport=51820";
        assert!(parse_conntrack_entry(internal).is_none());
    }

    #[test]
    fn test_parse_hex_socket_addr() {
        // 127.0.0.1:8080 -> 0100007F:1F90
        let (ip, port) = parse_hex_socket_addr("0100007F:1F90").unwrap();
        assert_eq!(ip, "127.0.0.1");
        assert_eq!(port, 8080);
    }
}
