use crate::net::telemetry::TelemetryCollector;
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
use std::io;
use std::time::Duration;

pub struct TuiDashboard;

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
        let mut collector = TelemetryCollector::new("wg0");

        loop {
            // Collect real kernel telemetry strictly on tunnel wg0
            let tele = collector.collect(&[]);
            let elapsed = tele.uptime_seconds;

            let packet_data: Vec<u64> = tele.traffic_history.clone();

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
                let status_style = if tele.status == "ONLINE" {
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                };

                let status_text = if tele.status == "ONLINE" {
                    "● LIVE KERNEL LINK (100% Online)"
                } else {
                    "● TUNNEL OFFLINE"
                };

                let title = Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(
                            " 🌐 WireNet ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw("── Real-Time Packet & Real IP Monitor (Rust Engine)"),
                    ]),
                    Line::from(vec![
                        Span::styled(" Status: ", Style::default().fg(Color::Gray)),
                        Span::styled(status_text, status_style),
                        Span::styled(
                            format!(
                                "  │  Uptime: {:02}:{:02}:{:02}  │  Active Player Connections: {}",
                                elapsed / 3600,
                                (elapsed % 3600) / 60,
                                elapsed % 60,
                                tele.active_players.len()
                            ),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                ])
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" WireNet Live Engine "),
                );
                f.render_widget(title, chunks[0]);

                // 2. Live Packet Throughput Sparkline & Gauge
                let sub_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                    .split(chunks[1]);

                let max_scale = (*packet_data.iter().max().unwrap_or(&10)).max(20);
                let sparkline = Sparkline::default()
                    .block(Block::default().borders(Borders::ALL).title(format!(
                        " Live Traffic: {} pkts/sec (Total: {}) ",
                        tele.interface.current_pps, tele.interface.total_packets
                    )))
                    .style(Style::default().fg(if tele.interface.current_pps > 0 {
                        Color::Green
                    } else {
                        Color::Cyan
                    }))
                    .data(&packet_data)
                    .max(max_scale);
                f.render_widget(sparkline, sub_chunks[0]);

                let gauge = Gauge::default()
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Tunnel Load Capacity "),
                    )
                    .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
                    .percent(tele.interface.load_percentage);
                f.render_widget(gauge, sub_chunks[1]);

                // 3. Protection Telemetry Stats
                let stats = Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(
                            " 🛡️  Shield Mode        : ",
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::styled(
                            &tele.protection.shield_mode,
                            Style::default().fg(Color::Green),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            " 🔒  Tunnel Interface   : ",
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::styled(
                            format!(
                                "{} Kernel Fastpath (Conntrack: {} states)",
                                tele.interface.name, tele.protection.conntrack_count
                            ),
                            Style::default().fg(Color::Cyan),
                        ),
                    ]),
                ])
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Live Protection Status "),
                );
                f.render_widget(stats, chunks[2]);

                // 4. Live Active Player Client IPs Table (100% Real IPs)
                let rows: Vec<Row> = if tele.active_players.is_empty() {
                    vec![Row::new(vec![
                        format!("Waiting for incoming packets on {}...", tele.interface.name),
                        "-".to_string(),
                        "Mapped Ports".to_string(),
                        "TCP / UDP".to_string(),
                        "● LISTENING ON FASTPATH".to_string(),
                    ])
                    .style(Style::default().fg(Color::DarkGray))]
                } else {
                    tele.active_players
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
                    Row::new(vec![
                        "Real Player IP",
                        "Src Port",
                        "Game Port",
                        "Protocol",
                        "Live Connection State",
                    ])
                    .style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Live Connected Player IPs (100% Real IP Stream) "),
                );
                f.render_widget(table, chunks[3]);

                // 5. Real-Time Packet & IP Event Log
                let log_items: Vec<ListItem> = if tele.packet_events.is_empty() {
                    vec![ListItem::new(Line::from(Span::styled(
                        "  ▶ [SYSTEM] WireNet Real-Time Packet Sniffer Active on wg0",
                        Style::default().fg(Color::DarkGray),
                    )))]
                } else {
                    tele.packet_events
                        .iter()
                        .map(|ev| {
                            let style = if ev.event_type == "CONNECTED" {
                                Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::BOLD)
                            } else if ev.event_type == "TRAFFIC" {
                                Style::default().fg(Color::Cyan)
                            } else {
                                Style::default().fg(Color::Yellow)
                            };
                            ListItem::new(Line::from(vec![
                                Span::styled(format!("  ▶ [{}] ", ev.timestamp), Style::default().fg(Color::DarkGray)),
                                Span::styled(format!("[{}] ", ev.event_type), style),
                                Span::styled(&ev.message, Style::default().fg(Color::White)),
                            ]))
                        })
                        .collect()
                };

                let list = List::new(log_items).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Real-Time Packet & IP Event Log "),
                );
                f.render_widget(list, chunks[4]);

                // 6. Footer
                let footer = Paragraph::new(
                    " Press [Q] or [Esc] to exit │ Live Kernel IP & Packet Sniffer Active ",
                )
                .style(Style::default().fg(Color::DarkGray));
                f.render_widget(footer, chunks[5]);
            })?;

            if event::poll(Duration::from_millis(500))? {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('q')
                        || key.code == KeyCode::Char('Q')
                        || key.code == KeyCode::Esc
                    {
                        return Ok(());
                    }
                }
            }
        }
    }
}
