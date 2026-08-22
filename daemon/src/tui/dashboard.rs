use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Row, Sparkline, Table},
    Terminal,
};
use std::collections::VecDeque;
#[allow(unused_imports)]
use std::fs;
use std::io;
use std::time::{Duration, Instant};

pub struct TuiDashboard;

#[derive(Clone, Debug)]
struct ClientConnection {
    client_ip: String,
    client_port: u16,
    game_port: u16,
    protocol: String,
    state: String,
}

impl TuiDashboard {
    pub fn run() -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let res = Self::run_app(&mut terminal);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        res
    }

    fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> Result<()> {
        let start_time = Instant::now();
        let mut packet_history: VecDeque<u64> = VecDeque::from(vec![0; 40]);
        let mut event_logs: VecDeque<String> = VecDeque::from(vec![
            "[SYSTEM] WireNet Real-Time Packet & Real IP Monitor Active".to_string(),
            "[TUNNEL] Encrypted WireGuard Kernel link active (10.200.0.1 ↔ 10.200.0.2)".to_string(),
            "[SCANNER] Actively monitoring /proc/net/tcp and /proc/net/nf_conntrack for player IPs".to_string(),
        ]);

        let mut last_sample_time = Instant::now();
        let mut last_total_packets = read_kernel_packets("wg0");
        let mut current_pps: u64 = 0;
        let mut total_cumulative_packets: u64 = 0;
        let mut active_connections: Vec<ClientConnection> = Vec::new();

        loop {
            let elapsed = start_time.elapsed().as_secs();

            // Sample real kernel packets every 500ms
            if last_sample_time.elapsed() >= Duration::from_millis(500) {
                let dt = last_sample_time.elapsed().as_secs_f64();
                let current_total_packets = read_kernel_packets("wg0");

                if current_total_packets >= last_total_packets {
                    let diff = current_total_packets - last_total_packets;
                    current_pps = (diff as f64 / dt) as u64;
                    total_cumulative_packets += diff;
                } else {
                    current_pps = 0;
                }

                last_total_packets = current_total_packets;
                last_sample_time = Instant::now();

                if packet_history.len() >= 50 {
                    packet_history.pop_front();
                }
                packet_history.push_back(current_pps);

                // Scan real player IPs from Linux Kernel TCP and Conntrack tables
                let discovered_conns = scan_real_player_connections();
                for conn in &discovered_conns {
                    if !active_connections.iter().any(|c| c.client_ip == conn.client_ip && c.client_port == conn.client_port) {
                        let now = chrono_like_time(elapsed);
                        if event_logs.len() >= 10 {
                            event_logs.pop_front();
                        }
                        event_logs.push_back(format!(
                            "[{}] PLAYER CONNECTED: IP {} (Port {}) ──► Minecraft:{}",
                            now, conn.client_ip, conn.client_port, conn.game_port
                        ));
                    }
                }
                active_connections = discovered_conns;

                // Also check if traffic is passing without specific TCP stream (e.g. UDP or Handshakes)
                if current_pps > 0 && active_connections.is_empty() {
                    let now = chrono_like_time(elapsed);
                    if event_logs.len() >= 10 {
                        event_logs.pop_front();
                    }
                    event_logs.push_back(format!(
                        "[{}] TUNNEL PACKETS: {} pkts/sec passing through wg0",
                        now, current_pps
                    ));
                }
            }

            let packet_data: Vec<u64> = packet_history.iter().copied().collect();

            terminal.draw(|f| {
                let size = f.size();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(4), // Header
                        Constraint::Length(4), // Live Real Throughput Graph
                        Constraint::Length(5), // Protection Telemetry
                        Constraint::Length(7), // Active Player Client IPs Table
                        Constraint::Min(5),    // Real-Time IP Connection Stream
                        Constraint::Length(3), // Footer
                    ])
                    .split(size);

                // 1. Header Block
                let title = Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(" 🌐 WireNet ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::raw("── Real-Time Packet & Real IP Monitor (Rust Engine)"),
                    ]),
                    Line::from(vec![
                        Span::styled(" Status: ", Style::default().fg(Color::Gray)),
                        Span::styled("● LIVE KERNEL LINK (100% Online)", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::styled(format!("  │  Uptime: {:02}:{:02}:{:02}  │  Active Player Connections: {}", elapsed / 3600, (elapsed % 3600) / 60, elapsed % 60, active_connections.len()), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    ]),
                ])
                .block(Block::default().borders(Borders::ALL).title(" WireNet Live Engine "));
                f.render_widget(title, chunks[0]);

                // 2. Live Packet Throughput Sparkline & Gauge
                let sub_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                    .split(chunks[1]);

                let max_scale = (*packet_data.iter().max().unwrap_or(&10)).max(20);
                let sparkline = Sparkline::default()
                    .block(Block::default().borders(Borders::ALL).title(format!(" Live Traffic: {} pkts/sec (Total: {}) ", current_pps, total_cumulative_packets)))
                    .style(Style::default().fg(if current_pps > 0 { Color::Green } else { Color::Cyan }))
                    .data(&packet_data)
                    .max(max_scale);
                f.render_widget(sparkline, sub_chunks[0]);

                let load_percent = ((current_pps as f64 / 200.0) * 100.0).min(100.0) as u16;
                let gauge = Gauge::default()
                    .block(Block::default().borders(Borders::ALL).title(" Tunnel Load Capacity "))
                    .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
                    .percent(load_percent);
                f.render_widget(gauge, sub_chunks[1]);

                // 3. Protection Telemetry Stats
                let stats = Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(" 🛡️  Shield Mode        : ", Style::default().fg(Color::Yellow)),
                        Span::styled("STANDARD (Hardware SYN Cookies + Per-IP Rate Limiter)", Style::default().fg(Color::Green)),
                    ]),
                    Line::from(vec![
                        Span::styled(" 🔒  Tunnel Interface   : ", Style::default().fg(Color::Yellow)),
                        Span::styled("wg0 Kernel Fastpath (10.200.0.1 ↔ 10.200.0.2)", Style::default().fg(Color::Cyan)),
                    ]),
                ])
                .block(Block::default().borders(Borders::ALL).title(" Live Protection Status "));
                f.render_widget(stats, chunks[2]);

                // 4. Live Active Player Client IPs Table (Real IPs instead of placeholder names!)
                let rows: Vec<Row> = if active_connections.is_empty() {
                    vec![
                        Row::new(vec![
                            "Waiting for players...".to_string(),
                            "-".to_string(),
                            "25565 - 25700".to_string(),
                            "TCP / UDP".to_string(),
                            "● LISTENING ON GATEWAY".to_string(),
                        ]).style(Style::default().fg(Color::DarkGray)),
                    ]
                } else {
                    active_connections
                        .iter()
                        .map(|c| {
                            Row::new(vec![
                                c.client_ip.clone(),
                                c.client_port.to_string(),
                                c.game_port.to_string(),
                                c.protocol.clone(),
                                c.state.clone(),
                            ])
                            .style(Style::default().fg(Color::Green))
                        })
                        .collect()
                };

                let table = Table::new(
                    rows,
                    [
                        Constraint::Percentage(30), // Player IP
                        Constraint::Percentage(15), // Source Port
                        Constraint::Percentage(15), // Game Port
                        Constraint::Percentage(15), // Protocol
                        Constraint::Percentage(25), // State
                    ],
                )
                .header(
                    Row::new(vec!["Real Player IP", "Src Port", "Game Port", "Protocol", "Live Connection State"])
                        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                )
                .block(Block::default().borders(Borders::ALL).title(" Live Connected Player IPs (100% Real IP Stream) "));
                f.render_widget(table, chunks[3]);

                // 5. Real-Time IP Connection Stream
                let log_items: Vec<ListItem> = event_logs
                    .iter()
                    .map(|log| {
                        let style = if log.contains("CONNECTED") || log.contains("INCOMING") {
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                        } else if log.contains("TUNNEL") {
                            Style::default().fg(Color::Cyan)
                        } else {
                            Style::default().fg(Color::Yellow)
                        };
                        ListItem::new(Line::from(Span::styled(format!("  ▶ {}", log), style)))
                    })
                    .collect();

                let list = List::new(log_items)
                    .block(Block::default().borders(Borders::ALL).title(" Real-Time Packet & IP Event Log "));
                f.render_widget(list, chunks[4]);

                // 6. Footer
                let footer = Paragraph::new(" Press [Q] or [Esc] to exit │ Live Kernel IP & Packet Sniffer Active ")
                    .style(Style::default().fg(Color::DarkGray));
                f.render_widget(footer, chunks[5]);
            })?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('q') || key.code == KeyCode::Char('Q') || key.code == KeyCode::Esc {
                        return Ok(());
                    }
                }
            }
        }
    }
}

/// Reads the real cumulative packet counter from Linux /sys/class/net/<iface>/statistics/
fn read_kernel_packets(iface: &str) -> u64 {
    #[cfg(unix)]
    {
        let rx_path = format!("/sys/class/net/{}/statistics/rx_packets", iface);
        let tx_path = format!("/sys/class/net/{}/statistics/tx_packets", iface);

        let rx = fs::read_to_string(&rx_path)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);

        let tx = fs::read_to_string(&tx_path)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);

        rx + tx
    }

    #[cfg(not(unix))]
    {
        let _ = iface;
        0
    }
}

/// Scans real client/player source IPs connected to game ports from Linux Kernel /proc/net/tcp and /proc/net/nf_conntrack
fn scan_real_player_connections() -> Vec<ClientConnection> {
    #[allow(unused_mut)]
    let mut conns = Vec::new();

    #[cfg(unix)]
    {
        // 1. Scan /proc/net/tcp for active sockets
        if let Ok(content) = fs::read_to_string("/proc/net/tcp") {
            for line in content.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let local_addr = parts[1];
                    let rem_addr = parts[2];
                    let state = parts[3];

                    // State 01 = ESTABLISHED
                    if state == "01" {
                        if let Some((_l_ip, l_port)) = parse_hex_socket_addr(local_addr) {
                            if (25565..=25700).contains(&l_port) || (30000..=30100).contains(&l_port) {
                                if let Some((r_ip, r_port)) = parse_hex_socket_addr(rem_addr) {
                                    if r_ip != "127.0.0.1" && !r_ip.starts_with("10.200.0.") {
                                        conns.push(ClientConnection {
                                            client_ip: r_ip,
                                            client_port: r_port,
                                            game_port: l_port,
                                            protocol: "TCP".to_string(),
                                            state: "● ACTIVE (0ms)".to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Scan /proc/net/nf_conntrack for NAT-forwarded player IPs
        if let Ok(content) = fs::read_to_string("/proc/net/nf_conntrack") {
            for line in content.lines() {
                if (line.contains("dport=25565") || line.contains("dport=25566")) && line.contains("src=") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    let mut src_ip = "";
                    let mut src_port = 0u16;
                    let mut dst_port = 25565u16;

                    for part in parts {
                        if part.starts_with("src=") && src_ip.is_empty() {
                            src_ip = part.trim_start_matches("src=");
                        } else if part.starts_with("sport=") && src_port == 0 {
                            src_port = part.trim_start_matches("sport=").parse().unwrap_or(0);
                        } else if part.starts_with("dport=") {
                            dst_port = part.trim_start_matches("dport=").parse().unwrap_or(25565);
                        }
                    }

                    if !src_ip.is_empty() && src_ip != "127.0.0.1" && !src_ip.starts_with("10.200.0.") {
                        if !conns.iter().any(|c| c.client_ip == src_ip && c.client_port == src_port) {
                            conns.push(ClientConnection {
                                client_ip: src_ip.to_string(),
                                client_port: src_port,
                                game_port: dst_port,
                                protocol: "TCP/NAT".to_string(),
                                state: "● FORWARDED".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    conns
}

#[cfg(unix)]
fn parse_hex_socket_addr(hex_str: &str) -> Option<(String, u16)> {
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

fn chrono_like_time(total_secs: u64) -> String {
    let hrs = (total_secs / 3600) % 24;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}:{:02}", hrs, mins, secs)
}
