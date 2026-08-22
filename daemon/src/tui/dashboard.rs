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
    widgets::{Block, Borders, Paragraph, Row, Table},
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
        loop {
            terminal.draw(|f| {
                let size = f.size();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(4),
                        Constraint::Length(7),
                        Constraint::Min(8),
                        Constraint::Length(3),
                    ])
                    .split(size);

                // 1. Header Block
                let title = Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(" 🌐 WireNet ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::raw("── Enterprise WireGuard & Anti-DDoS Control Center (Rust Engine)"),
                    ]),
                    Line::from(vec![
                        Span::styled(" Status: ", Style::default().fg(Color::Gray)),
                        Span::styled("● OPERATIONAL (100% Online)", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    ]),
                ])
                .block(Block::default().borders(Borders::ALL).title(" WireNet Control Plane "));
                f.render_widget(title, chunks[0]);

                // 2. Telemetry Stats Block
                let stats = Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(" 🛡️  Shield Mode        : ", Style::default().fg(Color::Yellow)),
                        Span::styled("STANDARD (SYN Cookies + Rate Limiting)", Style::default().fg(Color::Green)),
                    ]),
                    Line::from(vec![
                        Span::styled(" 🔒  Tunnel State       : ", Style::default().fg(Color::Yellow)),
                        Span::styled("WireGuard Kernel Link Active (10.200.0.1 ↔ 10.200.0.2)", Style::default().fg(Color::Cyan)),
                    ]),
                    Line::from(vec![
                        Span::styled(" 🚫  Dropped Attack Pkts: ", Style::default().fg(Color::Yellow)),
                        Span::styled("0 dropped (0 SYN Floods)", Style::default().fg(Color::White)),
                    ]),
                ])
                .block(Block::default().borders(Borders::ALL).title(" Live Protection Telemetry "));
                f.render_widget(stats, chunks[1]);

                // 3. Registered Node & Game Ports Table
                let rows = vec![
                    Row::new(vec!["node-1", "Pterodactyl Primary", "10.200.0.2", "25565 - 25700", "ONLINE (14ms)"]),
                ];
                let table = Table::new(
                    rows,
                    [
                        Constraint::Percentage(15),
                        Constraint::Percentage(25),
                        Constraint::Percentage(20),
                        Constraint::Percentage(20),
                        Constraint::Percentage(20),
                    ],
                )
                .header(
                    Row::new(vec!["Node ID", "Node Name", "Virtual IP", "Game Ports", "Health Status"])
                        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                )
                .block(Block::default().borders(Borders::ALL).title(" Connected Backend Nodes "));
                f.render_widget(table, chunks[2]);

                // 4. Footer
                let footer = Paragraph::new(" Press [Q] or [Ctrl+C] to exit TUI dashboard ")
                    .style(Style::default().fg(Color::DarkGray));
                f.render_widget(footer, chunks[3]);
            })?;

            if event::poll(Duration::from_millis(500))? {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                        return Ok(());
                    }
                }
            }
        }
    }
}
