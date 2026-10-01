use crate::controller::{BanInput, MappingInput, NodeInput, ServerInput, StateView, Store};
use crate::net::telemetry::{SystemTelemetry, TelemetryCollector};
use anyhow::{bail, Context, Result};
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    net::SocketAddr,
    path::Path,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use tokio::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemLogEntry {
    pub timestamp: String,
    pub level: String, // "INFO", "WARN", "ERROR", "SECURITY", "KERNEL", "MINECRAFT"
    pub source: String, // "DAEMON", "KERNEL", "MINECRAFT", "CONSOLE"
    pub message: String,
}

#[derive(Clone)]
struct AppState {
    store: Store,
    token: String,
    telemetry: Arc<Mutex<TelemetryCollector>>,
    logs: Arc<Mutex<VecDeque<SystemLogEntry>>>,
}

#[derive(Deserialize)]
pub struct CommandInput {
    pub command: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub password: Option<String>,
}

#[derive(Serialize)]
pub struct CommandOutput {
    pub success: bool,
    pub output: String,
    pub duration_ms: u64,
}

#[derive(Deserialize)]
pub struct DropConnectionInput {
    pub client_ip: String,
    pub client_port: u16,
    pub protocol: String,
}

#[derive(Deserialize)]
pub struct McStatusQuery {
    pub host: Option<String>,
    pub port: Option<u16>,
}

#[derive(Deserialize)]
pub struct McRconInput {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub command: String,
}

#[derive(Deserialize)]
pub struct LogQuery {
    pub level: Option<String>,
    pub limit: Option<usize>,
}

pub async fn serve(store: Store, listen: SocketAddr, token: String) -> Result<()> {
    let telemetry = Arc::new(Mutex::new(TelemetryCollector::new("wg0")));
    let mut initial_logs = VecDeque::with_capacity(500);
    initial_logs.push_back(SystemLogEntry {
        timestamp: current_time_string(),
        level: "INFO".to_string(),
        source: "DAEMON".to_string(),
        message: "WireNet Kernel Ingress Engine initialized successfully.".to_string(),
    });
    initial_logs.push_back(SystemLogEntry {
        timestamp: current_time_string(),
        level: "INFO".to_string(),
        source: "DAEMON".to_string(),
        message: "Pure Linux DNAT & Symmetric Return Policy Active (Zero-Proxy)".to_string(),
    });
    let logs = Arc::new(Mutex::new(initial_logs));

    let app = Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/telemetry", get(telemetry_handler))
        .route("/api/state", get(state))
        .route("/api/nodes", post(create_node))
        .route("/api/servers", post(create_server))
        .route("/api/mappings", post(create_mapping))
        .route("/api/mappings/:id/enable", post(enable_mapping))
        .route("/api/mappings/:id/disable", post(disable_mapping))
        .route("/api/bans", post(create_ban))
        .route("/api/bans/:id/delete", post(delete_ban))
        .route("/api/logs", get(logs_handler))
        .route("/api/commands/exec", post(exec_command_handler))
        .route("/api/connections/drop", post(drop_connection_handler))
        .route("/api/minecraft/status", get(minecraft_status_handler))
        .route("/api/minecraft/rcon", post(minecraft_rcon_handler))
        .route("/api/packets/sniff", get(sniff_packets_handler))
        .with_state(AppState {
            store,
            token,
            telemetry,
            logs,
        });

    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .context("binding dashboard listener")?;
    axum::serve(listener, app)
        .await
        .context("running dashboard")
}

pub fn load_token(path: &Path) -> Result<String> {
    let token = std::fs::read_to_string(path)
        .with_context(|| format!("reading dashboard token {}", path.display()))?;
    let token = token.trim().to_owned();
    if token.len() < 32 {
        bail!("dashboard token must contain at least 32 characters");
    }
    Ok(token)
}

pub fn create_token(path: &Path) -> Result<String> {
    if path.exists() {
        bail!(
            "refusing to overwrite existing dashboard token {}",
            path.display()
        );
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    std::fs::write(path, format!("{token}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(token)
}

pub fn validate_listen(listen: SocketAddr, allow_remote: bool) -> Result<()> {
    if !allow_remote && !listen.ip().is_loopback() {
        bail!("dashboard must bind to loopback unless --allow-remote is explicitly supplied");
    }
    Ok(())
}

pub fn install_service(
    database: &Path,
    token_file: &Path,
    listen: SocketAddr,
    allow_remote: bool,
) -> Result<()> {
    validate_listen(listen, allow_remote)?;
    let unit = format!(
        "[Unit]\n\
         Description=WireNet optional dashboard\n\
         After=network-online.target\n\
         Wants=network-online.target\n\n\
         [Service]\n\
         Type=simple\n\
         ExecStart=/usr/local/bin/wirenet dashboard serve --database {} --token-file {} --listen {}{}\n\
         Restart=on-failure\n\
         RestartSec=3\n\
         NoNewPrivileges=true\n\
         PrivateTmp=true\n\
         ProtectHome=true\n\n\
         [Install]\n\
         WantedBy=multi-user.target\n",
        database.display(),
        token_file.display(),
        listen,
        if allow_remote { " --allow-remote" } else { "" }
    );
    std::fs::write("/etc/systemd/system/wirenet-dashboard.service", unit)
        .context("writing dashboard unit")?;
    run_systemctl(&["daemon-reload"])
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    if enabled {
        run_systemctl(&["enable", "--now", "wirenet-dashboard.service"])
    } else {
        run_systemctl(&["disable", "--now", "wirenet-dashboard.service"])
    }
}

pub fn service_status() -> Result<String> {
    let out = Command::new("systemctl")
        .args(["is-active", "wirenet-dashboard.service"])
        .output()
        .context("checking dashboard service")?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn run_systemctl(args: &[&str]) -> Result<()> {
    let out = Command::new("systemctl")
        .args(args)
        .output()
        .context("running systemctl")?;
    if !out.status.success() {
        bail!(
            "systemctl {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(INDEX)
}

async fn health() -> &'static str {
    "ok"
}

async fn telemetry_handler(
    headers: HeaderMap,
    State(app): State<AppState>,
) -> ApiResult<Json<SystemTelemetry>> {
    authorize(&headers, &app)?;
    let mapped_ports: Vec<u16> = app
        .store
        .state()
        .map(|s| {
            s.mappings
                .into_iter()
                .filter(|m| m.enabled)
                .map(|m| m.public_port)
                .collect()
        })
        .unwrap_or_default();

    let mut collector = app.telemetry.lock().await;
    let tele = collector.collect(&mapped_ports);
    Ok(Json(tele))
}

async fn state(headers: HeaderMap, State(app): State<AppState>) -> ApiResult<Json<StateView>> {
    authorize(&headers, &app)?;
    Ok(Json(app.store.state().map_err(ApiError::from)?))
}

async fn create_node(
    headers: HeaderMap,
    State(app): State<AppState>,
    Json(input): Json<NodeInput>,
) -> ApiResult<StatusCode> {
    authorize(&headers, &app)?;
    app.store
        .add_node(input, "dashboard")
        .map_err(ApiError::from)?;
    Ok(StatusCode::CREATED)
}

async fn create_server(
    headers: HeaderMap,
    State(app): State<AppState>,
    Json(input): Json<ServerInput>,
) -> ApiResult<StatusCode> {
    authorize(&headers, &app)?;
    app.store
        .add_server(input, "dashboard")
        .map_err(ApiError::from)?;
    Ok(StatusCode::CREATED)
}

async fn create_mapping(
    headers: HeaderMap,
    State(app): State<AppState>,
    Json(input): Json<MappingInput>,
) -> ApiResult<StatusCode> {
    authorize(&headers, &app)?;
    app.store
        .add_mapping(input, "dashboard")
        .map_err(ApiError::from)?;
    Ok(StatusCode::CREATED)
}

async fn enable_mapping(
    headers: HeaderMap,
    State(app): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<StatusCode> {
    authorize(&headers, &app)?;
    app.store
        .toggle_mapping(&id, true, "dashboard")
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn disable_mapping(
    headers: HeaderMap,
    State(app): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<StatusCode> {
    authorize(&headers, &app)?;
    app.store
        .toggle_mapping(&id, false, "dashboard")
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_ban(
    headers: HeaderMap,
    State(app): State<AppState>,
    Json(input): Json<BanInput>,
) -> ApiResult<StatusCode> {
    authorize(&headers, &app)?;
    app.store
        .add_ban(input, "dashboard")
        .map_err(ApiError::from)?;
    Ok(StatusCode::CREATED)
}

async fn delete_ban(
    headers: HeaderMap,
    State(app): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> ApiResult<StatusCode> {
    authorize(&headers, &app)?;
    app.store
        .remove_ban(id, "dashboard")
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn logs_handler(
    headers: HeaderMap,
    State(app): State<AppState>,
    Query(query): Query<LogQuery>,
) -> ApiResult<Json<Vec<SystemLogEntry>>> {
    authorize(&headers, &app)?;
    let logs = app.logs.lock().await;
    let limit = query.limit.unwrap_or(100).min(500);
    let level_filter = query
        .level
        .unwrap_or_else(|| "ALL".to_string())
        .to_uppercase();

    let mut filtered: Vec<SystemLogEntry> = logs
        .iter()
        .filter(|e| level_filter == "ALL" || e.level.to_uppercase() == level_filter)
        .rev()
        .take(limit)
        .cloned()
        .collect();
    filtered.reverse();
    Ok(Json(filtered))
}

async fn exec_command_handler(
    headers: HeaderMap,
    State(app): State<AppState>,
    Json(input): Json<CommandInput>,
) -> ApiResult<Json<CommandOutput>> {
    authorize(&headers, &app)?;
    let start = Instant::now();
    let cmd = input.command.trim();

    // Check if it's an RCON command
    if cmd.starts_with("mc:")
        || cmd.starts_with("rcon:")
        || (input.password.is_some() && !cmd.is_empty())
    {
        let rcon_cmd = cmd
            .strip_prefix("mc:")
            .or_else(|| cmd.strip_prefix("rcon:"))
            .unwrap_or(cmd)
            .trim();
        let host = input.host.as_deref().unwrap_or("127.0.0.1");
        let port = input.port.unwrap_or(25575);
        let pass = input.password.as_deref().unwrap_or("");

        match crate::protocol::minecraft::exec_rcon(
            host,
            port,
            pass,
            rcon_cmd,
            Duration::from_secs(4),
        )
        .await
        {
            Ok(output) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                log_event(
                    &app,
                    "INFO",
                    "MINECRAFT",
                    &format!("RCON [{host}:{port}]: {rcon_cmd}"),
                )
                .await;
                return Ok(Json(CommandOutput {
                    success: true,
                    output,
                    duration_ms,
                }));
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                log_event(
                    &app,
                    "WARN",
                    "MINECRAFT",
                    &format!("RCON error [{host}:{port}]: {e:#}"),
                )
                .await;
                return Ok(Json(CommandOutput {
                    success: false,
                    output: format!("RCON Error: {e:#}"),
                    duration_ms,
                }));
            }
        }
    }

    // Execute system / WireNet CLI command
    #[cfg(unix)]
    let out = Command::new("sh").args(["-c", cmd]).output();
    #[cfg(windows)]
    let out = Command::new("cmd").args(["/C", cmd]).output();

    let duration_ms = start.elapsed().as_millis() as u64;
    match out {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).to_string();
            let mut output = stdout;
            if !stderr.is_empty() {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(&stderr);
            }
            if output.is_empty() && o.status.success() {
                output = "Command executed successfully (no output).".to_string();
            }

            let log_lvl = if o.status.success() { "INFO" } else { "WARN" };
            log_event(
                &app,
                log_lvl,
                "CONSOLE",
                &format!("Executed `{cmd}` ({duration_ms}ms)"),
            )
            .await;

            Ok(Json(CommandOutput {
                success: o.status.success(),
                output,
                duration_ms,
            }))
        }
        Err(e) => {
            log_event(&app, "ERROR", "CONSOLE", &format!("Failed `{cmd}`: {e:#}")).await;
            Ok(Json(CommandOutput {
                success: false,
                output: format!("Execution failed: {e:#}"),
                duration_ms,
            }))
        }
    }
}

async fn drop_connection_handler(
    headers: HeaderMap,
    State(app): State<AppState>,
    Json(input): Json<DropConnectionInput>,
) -> ApiResult<StatusCode> {
    authorize(&headers, &app)?;
    crate::net::telemetry::drop_connection(&input.client_ip, input.client_port, &input.protocol)
        .map_err(ApiError::from)?;
    log_event(
        &app,
        "SECURITY",
        "KERNEL",
        &format!(
            "Terminated active flow {}:{} ({})",
            input.client_ip, input.client_port, input.protocol
        ),
    )
    .await;
    Ok(StatusCode::OK)
}

async fn minecraft_status_handler(
    headers: HeaderMap,
    State(app): State<AppState>,
    Query(query): Query<McStatusQuery>,
) -> ApiResult<Json<crate::protocol::minecraft::MinecraftStatus>> {
    authorize(&headers, &app)?;
    let host = query.host.unwrap_or_else(|| "127.0.0.1".to_string());
    let port = query.port.unwrap_or(25565);
    let status = crate::protocol::minecraft::query_minecraft_status(&host, port).await;
    Ok(Json(status))
}

async fn minecraft_rcon_handler(
    headers: HeaderMap,
    State(app): State<AppState>,
    Json(input): Json<McRconInput>,
) -> ApiResult<Json<CommandOutput>> {
    authorize(&headers, &app)?;
    let start = Instant::now();
    match crate::protocol::minecraft::exec_rcon(
        &input.host,
        input.port,
        &input.password,
        &input.command,
        Duration::from_secs(4),
    )
    .await
    {
        Ok(out) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            log_event(
                &app,
                "INFO",
                "MINECRAFT",
                &format!("RCON [{}]: {}", input.command, out.trim()),
            )
            .await;
            Ok(Json(CommandOutput {
                success: true,
                output: out,
                duration_ms,
            }))
        }
        Err(e) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            log_event(&app, "WARN", "MINECRAFT", &format!("RCON Error: {:?}", e)).await;
            Ok(Json(CommandOutput {
                success: false,
                output: format!("RCON Error: {:#}", e),
                duration_ms,
            }))
        }
    }
}

async fn sniff_packets_handler(
    headers: HeaderMap,
    State(app): State<AppState>,
) -> ApiResult<Json<Vec<String>>> {
    authorize(&headers, &app)?;
    let packets = crate::net::telemetry::sniff_interface_packets("wg0", 25).unwrap_or_default();
    Ok(Json(packets))
}

async fn log_event(app: &AppState, level: &str, source: &str, msg: &str) {
    let now = current_time_string();
    let mut logs = app.logs.lock().await;
    if logs.len() >= 500 {
        logs.pop_front();
    }
    logs.push_back(SystemLogEntry {
        timestamp: now,
        level: level.to_string(),
        source: source.to_string(),
        message: msg.to_string(),
    });
}

fn current_time_string() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let hrs = (now / 3600) % 24;
    let mins = (now % 3600) / 60;
    let secs = now % 60;
    format!("{:02}:{:02}:{:02}", hrs, mins, secs)
}

fn authorize(headers: &HeaderMap, app: &AppState) -> ApiResult<()> {
    let provided = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if provided.as_bytes().ct_eq(app.token.as_bytes()).into() {
        Ok(())
    } else {
        Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "dashboard authentication required".into(),
        ))
    }
}

type ApiResult<T> = std::result::Result<T, ApiError>;
struct ApiError(StatusCode, String);
impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self(StatusCode::BAD_REQUEST, e.to_string())
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.0, self.1).into_response()
    }
}

const INDEX: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>WireNet — High-Performance Kernel Control Center</title>
<style>
:root {
  --bg: #070b14;
  --surface: #0e1526;
  --card: #131c33;
  --card-border: #1e293b;
  --card-hover: #182340;
  --text: #f8fafc;
  --text-muted: #94a3b8;
  --cyan: #00f0ff;
  --green: #10b981;
  --amber: #f59e0b;
  --rose: #ef4444;
  --purple: #a855f7;
  --mono: 'JetBrains Mono', 'Fira Code', 'Courier New', monospace;
}
* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  background: var(--bg);
  color: var(--text);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
  line-height: 1.5;
  padding: 1.5rem;
  max-width: 1450px;
  margin: 0 auto;
}
/* Header */
header {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--card-border);
  border-radius: 12px;
  padding: 1rem 1.5rem;
  margin-bottom: 1.25rem;
  box-shadow: 0 4px 20px rgba(0,0,0,0.4);
}
.brand {
  display: flex;
  align-items: center;
  gap: 0.85rem;
}
.brand h1 {
  font-size: 1.35rem;
  font-weight: 700;
  color: #fff;
  letter-spacing: -0.3px;
}
.badge-version {
  background: rgba(0,240,255,0.15);
  color: var(--cyan);
  font-size: 0.75rem;
  padding: 0.2rem 0.6rem;
  border-radius: 20px;
  font-family: var(--mono);
  font-weight: 600;
}
.auth-bar {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.input-token {
  background: #050811;
  border: 1px solid var(--card-border);
  color: #fff;
  padding: 0.45rem 0.75rem;
  border-radius: 6px;
  font-family: var(--mono);
  font-size: 0.85rem;
  width: 260px;
}
.input-token:focus { outline: none; border-color: var(--cyan); }
.btn {
  background: var(--cyan);
  color: #000;
  border: none;
  font-weight: 600;
  font-size: 0.85rem;
  padding: 0.45rem 1rem;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
}
.btn:hover { filter: brightness(1.15); transform: translateY(-1px); }
.btn-secondary {
  background: var(--card);
  color: var(--text);
  border: 1px solid var(--card-border);
}
.btn-secondary:hover { background: var(--card-hover); color: #fff; }
.btn-danger {
  background: rgba(239, 68, 68, 0.2);
  color: var(--rose);
  border: 1px solid var(--rose);
}
.btn-danger:hover { background: var(--rose); color: #fff; }
.btn-success {
  background: rgba(16, 185, 129, 0.2);
  color: var(--green);
  border: 1px solid var(--green);
}
.status-pill {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.25rem 0.75rem;
  border-radius: 20px;
  font-size: 0.8rem;
  font-weight: 600;
}
.status-pill.online {
  background: rgba(16, 185, 129, 0.15);
  color: var(--green);
  border: 1px solid rgba(16, 185, 129, 0.3);
}
.status-pill.offline {
  background: rgba(239, 68, 68, 0.15);
  color: var(--rose);
  border: 1px solid rgba(239, 68, 68, 0.3);
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: currentColor;
  box-shadow: 0 0 8px currentColor;
}
/* Navigation Tabs */
nav {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
  margin-bottom: 1.5rem;
  border-bottom: 1px solid var(--card-border);
  padding-bottom: 0.5rem;
}
.tab-btn {
  background: transparent;
  color: var(--text-muted);
  border: none;
  font-size: 0.9rem;
  font-weight: 600;
  padding: 0.5rem 0.9rem;
  cursor: pointer;
  border-radius: 6px;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
}
.tab-btn:hover {
  color: #fff;
  background: rgba(255,255,255,0.05);
}
.tab-btn.active {
  color: var(--cyan);
  background: rgba(0,240,255,0.1);
  border-bottom: 2px solid var(--cyan);
}
/* Metrics Grid */
.metrics-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
  gap: 1rem;
  margin-bottom: 1.5rem;
}
.card {
  background: var(--surface);
  border: 1px solid var(--card-border);
  border-radius: 12px;
  padding: 1.25rem;
  box-shadow: 0 4px 15px rgba(0,0,0,0.25);
  transition: border-color 0.2s;
}
.card:hover { border-color: rgba(0,240,255,0.3); }
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.75rem;
}
.card-title {
  font-size: 0.8rem;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--text-muted);
  letter-spacing: 0.5px;
}
.card-value {
  font-size: 1.75rem;
  font-weight: 700;
  font-family: var(--mono);
  color: #fff;
}
.card-subtext {
  font-size: 0.8rem;
  color: var(--text-muted);
  margin-top: 0.35rem;
  font-family: var(--mono);
}
.sparkline-container {
  margin-top: 0.75rem;
  height: 45px;
  display: flex;
  align-items: flex-end;
}
.progress-bar-bg {
  background: #050811;
  border-radius: 6px;
  height: 8px;
  overflow: hidden;
  margin-top: 0.75rem;
  border: 1px solid var(--card-border);
}
.progress-bar-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--cyan), var(--green));
  width: 0%;
  transition: width 0.3s ease;
}
/* Tables */
table {
  width: 100%;
  border-collapse: collapse;
  margin-top: 0.5rem;
}
th {
  background: #0a0f1d;
  color: var(--text-muted);
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  padding: 0.75rem 1rem;
  text-align: left;
  border-bottom: 1px solid var(--card-border);
}
td {
  padding: 0.75rem 1rem;
  font-size: 0.85rem;
  border-bottom: 1px solid rgba(255,255,255,0.03);
}
tr:hover td { background: rgba(255,255,255,0.02); }
.mono { font-family: var(--mono); }
.tag-proto {
  background: rgba(0,240,255,0.15);
  color: var(--cyan);
  padding: 0.15rem 0.5rem;
  border-radius: 4px;
  font-size: 0.75rem;
  font-weight: 600;
}
.tag-state {
  background: rgba(16,185,129,0.15);
  color: var(--green);
  padding: 0.15rem 0.5rem;
  border-radius: 4px;
  font-size: 0.75rem;
  font-weight: 600;
}
/* Terminal & Log Viewer */
.log-terminal {
  background: #04060c;
  border: 1px solid var(--card-border);
  border-radius: 8px;
  padding: 1rem;
  height: 380px;
  overflow-y: auto;
  font-family: var(--mono);
  font-size: 0.82rem;
  line-height: 1.6;
}
.log-line {
  display: flex;
  gap: 0.75rem;
  margin-bottom: 0.25rem;
  word-break: break-all;
}
.log-time { color: #64748b; }
.log-source { color: #94a3b8; font-weight: 600; min-width: 90px; }
.log-lvl-info { color: var(--cyan); }
.log-lvl-warn { color: var(--amber); }
.log-lvl-error { color: var(--rose); font-weight: 700; }
.log-lvl-security { color: var(--purple); font-weight: 700; }
.log-lvl-packet { color: var(--green); }
.log-msg { color: #e2e8f0; }

/* Interactive Command Console */
.console-box {
  background: #030509;
  border: 1px solid var(--card-border);
  border-radius: 8px;
  height: 340px;
  overflow-y: auto;
  padding: 1rem;
  font-family: var(--mono);
  font-size: 0.85rem;
  color: #38bdf8;
  white-space: pre-wrap;
  line-height: 1.5;
}
.console-prompt-bar {
  display: flex;
  gap: 0.5rem;
  margin-top: 0.75rem;
  align-items: center;
}
.console-input {
  flex: 1;
  background: #050811;
  border: 1px solid var(--card-border);
  color: #fff;
  padding: 0.6rem 0.9rem;
  border-radius: 6px;
  font-family: var(--mono);
  font-size: 0.9rem;
}
.console-input:focus { outline: none; border-color: var(--cyan); }
.quick-ops-bar {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
  margin-bottom: 0.75rem;
}

/* Minecraft Server Cards */
.mc-cards-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
  gap: 1.25rem;
}
.mc-card {
  background: var(--surface);
  border: 1px solid var(--card-border);
  border-radius: 12px;
  padding: 1.25rem;
  box-shadow: 0 4px 15px rgba(0,0,0,0.3);
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}
.mc-card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.mc-title { font-size: 1.05rem; font-weight: 700; color: #fff; }
.mc-motd-box {
  background: #060912;
  border: 1px solid var(--card-border);
  border-radius: 6px;
  padding: 0.75rem;
  font-family: var(--mono);
  font-size: 0.85rem;
  min-height: 48px;
  color: #cbd5e1;
}
.mc-stat-row {
  display: flex;
  justify-content: space-between;
  font-size: 0.85rem;
  color: var(--text-muted);
}
.mc-rcon-input-group {
  display: flex;
  gap: 0.5rem;
  margin-top: 0.5rem;
}

/* Forms */
.form-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 0.75rem;
  margin-top: 0.75rem;
}
.form-input {
  background: #050811;
  border: 1px solid var(--card-border);
  color: #fff;
  padding: 0.5rem 0.75rem;
  border-radius: 6px;
  font-size: 0.85rem;
  font-family: var(--mono);
}
.form-input:focus { outline: none; border-color: var(--cyan); }
.alert {
  padding: 0.75rem 1rem;
  border-radius: 8px;
  margin-bottom: 1rem;
  font-size: 0.85rem;
  display: none;
}
.alert-error { background: rgba(239,68,68,0.2); color: #fca5a5; border: 1px solid rgba(239,68,68,0.4); }
.alert-success { background: rgba(16,185,129,0.2); color: #6ee7b7; border: 1px solid rgba(16,185,129,0.4); }
.hidden { display: none; }
</style>
</head>
<body>

<header>
  <div class="brand">
    <span style="font-size: 1.8rem">🌐</span>
    <div>
      <div style="display:flex; align-items:center; gap:0.5rem">
        <h1>WireNet Control Center</h1>
        <span class="badge-version">Pure Kernel Engine</span>
      </div>
      <div style="font-size:0.8rem; color:var(--text-muted); margin-top:2px">
        Real-Time Zero-Proxy Ingress • Authentic Client IP Delivery • Kernel Telemetry
      </div>
    </div>
  </div>

  <div class="auth-bar">
    <span id="connStatusPill" class="status-pill offline">
      <span class="dot"></span> <span id="connStatusText">Connecting...</span>
    </span>
    <input id="authToken" class="input-token" type="password" placeholder="Dashboard Bearer Token...">
    <button class="btn" onclick="saveAndConnect()">Connect</button>
  </div>
</header>

<div id="alertBox" class="alert"></div>

<nav>
  <button class="tab-btn active" onclick="switchTab('monitor')">📊 Live Monitor & Real IP Stream</button>
  <button class="tab-btn" onclick="switchTab('minecraft')">🎮 Minecraft Servers & RCON</button>
  <button class="tab-btn" onclick="switchTab('mappings')">🔀 Port Mappings</button>
  <button class="tab-btn" onclick="switchTab('nodes')">🖥️ Nodes & Servers</button>
  <button class="tab-btn" onclick="switchTab('security')">🛡️ Anti-DDoS & IP Bans</button>
  <button class="tab-btn" onclick="switchTab('logs')">📜 Real-Time Logs</button>
  <button class="tab-btn" onclick="switchTab('console')">💻 Command Console</button>
</nav>

<!-- Tab 1: Live Monitor -->
<div id="tab-monitor">
  <div class="metrics-grid">
    <div class="card">
      <div class="card-header">
        <span class="card-title">Kernel Link Status</span>
        <span id="linkPill" class="status-pill online"><span class="dot"></span> ONLINE</span>
      </div>
      <div id="metricInterface" class="card-value">wg0</div>
      <div id="metricUptime" class="card-subtext">Uptime: 00:00:00</div>
    </div>

    <div class="card">
      <div class="card-header">
        <span class="card-title">Tunnel Throughput (wg0)</span>
        <span style="color:var(--cyan); font-weight:600; font-family:var(--mono)" id="livePps">0 pkts/s</span>
      </div>
      <div id="metricTotalPackets" class="card-value">0</div>
      <div id="metricBytes" class="card-subtext">RX: 0 B │ TX: 0 B</div>
      <div class="sparkline-container">
        <svg id="sparklineSvg" width="100%" height="45" style="overflow:visible">
          <polyline id="sparklinePoly" fill="none" stroke="var(--cyan)" stroke-width="2" points="0,45 300,45" />
        </svg>
      </div>
    </div>

    <div class="card">
      <div class="card-header">
        <span class="card-title">Tunnel Load Capacity</span>
        <span id="loadPercentText" style="color:var(--green); font-weight:600; font-family:var(--mono)">0%</span>
      </div>
      <div id="activePlayersCount" class="card-value">0 Players</div>
      <div id="peerHandshake" class="card-subtext">Peer: Active Handshake</div>
      <div class="progress-bar-bg">
        <div id="loadProgressBar" class="progress-bar-fill"></div>
      </div>
    </div>

    <div class="card">
      <div class="card-header">
        <span class="card-title">Anti-DDoS Shield</span>
        <span style="color:var(--green); font-weight:600">ACTIVE</span>
      </div>
      <div id="metricShield" class="card-value" style="font-size:1.2rem">STANDARD</div>
      <div id="metricConntrack" class="card-subtext">Conntrack: 0 states</div>
    </div>
  </div>

  <!-- Live Connected Player IPs Table (100% Real IP Stream) -->
  <div class="card" style="margin-bottom: 1.5rem">
    <div class="card-header">
      <div>
        <h2 style="font-size:1.1rem; font-weight:700">Live Connected Player IPs (100% Real IP Stream)</h2>
        <p style="font-size:0.8rem; color:var(--text-muted); margin-top:2px">
          Authentic client source IPs parsed directly from Linux Kernel /proc/net/nf_conntrack & TCP sockets (Zero proxy masking)
        </p>
      </div>
      <button class="btn btn-secondary" style="font-size:0.75rem" onclick="poll()">⟳ Refresh Now</button>
    </div>
    <table>
      <thead>
        <tr>
          <th>Real Player IP</th>
          <th>Source Port</th>
          <th>Target Game Port</th>
          <th>Protocol</th>
          <th>Connection State</th>
          <th>Transferred</th>
          <th>Control Actions</th>
        </tr>
      </thead>
      <tbody id="playerTableBody">
        <tr><td colspan="7" style="text-align:center; color:var(--text-muted)">Waiting for player connection packets on wg0...</td></tr>
      </tbody>
    </table>
  </div>

  <!-- Real-Time Packet & IP Event Log -->
  <div class="card">
    <div class="card-header">
      <div>
        <h2 style="font-size:1.1rem; font-weight:700">Real-Time Packet & IP Event Stream</h2>
        <p style="font-size:0.8rem; color:var(--text-muted); margin-top:2px">
          Kernel packet arrivals, player handshake events, and connection lifecycle
        </p>
      </div>
      <div style="display:flex; gap:0.5rem">
        <button id="pauseBtn" class="btn btn-secondary" style="font-size:0.75rem" onclick="togglePause()">Pause Stream</button>
        <button class="btn btn-secondary" style="font-size:0.75rem" onclick="clearLogs()">Clear</button>
      </div>
    </div>
    <div id="eventLogContainer" class="log-terminal" style="height:200px">
      <div class="log-line"><span class="log-time">[SYSTEM]</span> <span class="log-msg">WireNet Real-Time Packet Sniffer Active. Listening on tunnel fastpath...</span></div>
    </div>
  </div>
</div>

<!-- Tab 2: Minecraft Servers & RCON -->
<div id="tab-minecraft" class="hidden">
  <div class="card" style="margin-bottom: 1.5rem">
    <div class="card-header">
      <div>
        <h2 style="font-size:1.15rem; font-weight:700">🎮 Genuine Minecraft Server Status & Ping</h2>
        <p style="font-size:0.85rem; color:var(--text-muted)">
          Direct Server List Ping (SLP) for Java and RakNet Unconnected Ping for Bedrock. No fake simulation.
        </p>
      </div>
      <button class="btn btn-secondary" style="font-size:0.75rem" onclick="refreshMinecraftServers()">⟳ Refresh All Statuses</button>
    </div>
    <div id="mcCardsGrid" class="mc-cards-grid" style="margin-top:0.75rem">
      <div style="color:var(--text-muted); padding:1rem">No configured server mappings to inspect. Add mappings in the Port Mappings tab.</div>
    </div>
  </div>

  <!-- On-Demand Manual Ping Inspector -->
  <div class="card">
    <h2 style="font-size:1.1rem; font-weight:700; margin-bottom:0.5rem">🔍 Manual Minecraft SLP Ping Probe</h2>
    <p style="font-size:0.85rem; color:var(--text-muted)">Send an authentic Server List Ping or RakNet query to any target host and port.</p>
    <div class="form-grid">
      <input id="probe_host" class="form-input" placeholder="Host / IP (e.g. 127.0.0.1)" value="127.0.0.1">
      <input id="probe_port" class="form-input" type="number" placeholder="Port (e.g. 25565)" value="25565">
      <button class="btn" onclick="runManualProbe()">Ping Server</button>
    </div>
    <div id="probeResultBox" style="margin-top:1rem; display:none" class="mc-motd-box"></div>
  </div>
</div>

<!-- Tab 3: Port Mappings -->
<div id="tab-mappings" class="hidden">
  <div class="card" style="margin-bottom: 1.5rem">
    <h2 style="font-size:1.1rem; font-weight:700; margin-bottom:0.5rem">Reserve New Ingress Mapping</h2>
    <p style="font-size:0.85rem; color:var(--text-muted)">Maps an external public IP and port to a private Pterodactyl backend via Layer-3 DNAT.</p>
    <div class="form-grid">
      <input id="m_id" class="form-input" placeholder="Mapping ID (e.g. map-mc-01)">
      <input id="m_server" class="form-input" placeholder="Server ID (e.g. srv-01)">
      <input id="m_ip" class="form-input" placeholder="Public IPv4 (e.g. 198.51.100.10)">
      <input id="m_port" class="form-input" type="number" placeholder="Public Port (e.g. 25565)">
      <input id="m_bport" class="form-input" type="number" placeholder="Backend Port (e.g. 25565)">
      <select id="m_proto" class="form-input">
        <option value="tcp">TCP (Minecraft Java / Standard)</option>
        <option value="udp">UDP (Minecraft Bedrock / Voice Chat)</option>
      </select>
    </div>
    <button class="btn" style="margin-top:1rem" onclick="submitMapping()">Reserve Mapping</button>
  </div>

  <div class="card">
    <h2 style="font-size:1.1rem; font-weight:700; margin-bottom:0.75rem">Active Ingress Mappings</h2>
    <table>
      <thead>
        <tr>
          <th>Mapping ID</th>
          <th>Server ID</th>
          <th>Public Endpoint</th>
          <th>Backend Port</th>
          <th>Protocol</th>
          <th>Status</th>
          <th>Actions</th>
        </tr>
      </thead>
      <tbody id="mappingsTableBody">
        <tr><td colspan="7" style="text-align:center; color:var(--text-muted)">No ingress mappings configured.</td></tr>
      </tbody>
    </table>
  </div>
</div>

<!-- Tab 4: Nodes & Servers -->
<div id="tab-nodes" class="hidden">
  <div class="card" style="margin-bottom: 1.5rem">
    <h2 style="font-size:1.1rem; font-weight:700; margin-bottom:0.5rem">Enroll Node</h2>
    <div class="form-grid">
      <input id="n_id" class="form-input" placeholder="Node ID (e.g. node-ptero-01)">
      <input id="n_name" class="form-input" placeholder="Node Name (e.g. EU-Node-1)">
      <input id="n_ip" class="form-input" placeholder="Tunnel IP (e.g. 10.200.0.2)">
      <input id="n_key" class="form-input" placeholder="WireGuard Public Key">
    </div>
    <button class="btn" style="margin-top:1rem" onclick="submitNode()">Add Node</button>
  </div>

  <div class="card" style="margin-bottom: 1.5rem">
    <h2 style="font-size:1.1rem; font-weight:700; margin-bottom:0.75rem">Enrolled Nodes</h2>
    <table>
      <thead>
        <tr>
          <th>Node ID</th>
          <th>Name</th>
          <th>Virtual IP</th>
          <th>WireGuard Key</th>
        </tr>
      </thead>
      <tbody id="nodesTableBody">
        <tr><td colspan="4" style="text-align:center; color:var(--text-muted)">No nodes enrolled.</td></tr>
      </tbody>
    </table>
  </div>

  <div class="card" style="margin-bottom: 1.5rem">
    <h2 style="font-size:1.1rem; font-weight:700; margin-bottom:0.5rem">Register Pterodactyl Server</h2>
    <div class="form-grid">
      <input id="s_id" class="form-input" placeholder="Server ID (e.g. srv-mc-01)">
      <input id="s_node" class="form-input" placeholder="Node ID (e.g. node-ptero-01)">
      <input id="s_cust" class="form-input" placeholder="Customer ID (e.g. cust-88)">
      <input id="s_ptero" class="form-input" placeholder="Pterodactyl UUID">
    </div>
    <button class="btn" style="margin-top:1rem" onclick="submitServer()">Register Server</button>
  </div>

  <div class="card">
    <h2 style="font-size:1.1rem; font-weight:700; margin-bottom:0.75rem">Registered Servers</h2>
    <table>
      <thead>
        <tr>
          <th>Server ID</th>
          <th>Node ID</th>
          <th>Customer ID</th>
          <th>Pterodactyl UUID</th>
        </tr>
      </thead>
      <tbody id="serversTableBody">
        <tr><td colspan="4" style="text-align:center; color:var(--text-muted)">No servers registered.</td></tr>
      </tbody>
    </table>
  </div>
</div>

<!-- Tab 5: Security & Bans -->
<div id="tab-security" class="hidden">
  <div class="card" style="margin-bottom: 1.5rem">
    <h2 style="font-size:1.1rem; font-weight:700; margin-bottom:0.5rem">Add Source IP Ban</h2>
    <p style="font-size:0.85rem; color:var(--text-muted)">Durable or timed kernel-level drop rule for malicious source IPs.</p>
    <div class="form-grid">
      <input id="b_ip" class="form-input" placeholder="Banned IP (e.g. 198.51.100.99)">
      <input id="b_reason" class="form-input" placeholder="Reason (e.g. Bot Attack)">
      <input id="b_mapping" class="form-input" placeholder="Optional Mapping ID">
      <input id="b_exp" class="form-input" type="number" placeholder="Expiry in seconds (optional)">
    </div>
    <button class="btn btn-danger" style="margin-top:1rem" onclick="submitBan()">Apply IP Ban</button>
  </div>

  <div class="card">
    <h2 style="font-size:1.1rem; font-weight:700; margin-bottom:0.75rem">Active IP Bans</h2>
    <table>
      <thead>
        <tr>
          <th>ID</th>
          <th>Banned Source IP</th>
          <th>Reason</th>
          <th>Mapping</th>
          <th>Created</th>
          <th>Actions</th>
        </tr>
      </thead>
      <tbody id="bansTableBody">
        <tr><td colspan="6" style="text-align:center; color:var(--text-muted)">No active bans.</td></tr>
      </tbody>
    </table>
  </div>
</div>

<!-- Tab 6: Real-Time Logs -->
<div id="tab-logs" class="hidden">
  <div class="card">
    <div class="card-header">
      <div>
        <h2 style="font-size:1.15rem; font-weight:700">📜 System, Daemon & Kernel Logs</h2>
        <p style="font-size:0.85rem; color:var(--text-muted)">Real-time streaming log buffer with level filtering and export.</p>
      </div>
      <div style="display:flex; gap:0.5rem; align-items:center">
        <select id="logLevelSelect" class="form-input" style="padding:0.3rem 0.6rem; font-size:0.8rem" onchange="fetchLogs()">
          <option value="ALL">All Levels</option>
          <option value="INFO">INFO</option>
          <option value="WARN">WARN</option>
          <option value="ERROR">ERROR</option>
          <option value="SECURITY">SECURITY</option>
        </select>
        <button class="btn btn-secondary" style="font-size:0.75rem" onclick="fetchLogs()">⟳ Refresh</button>
        <button class="btn btn-secondary" style="font-size:0.75rem" onclick="downloadLogs()">⬇ Export</button>
      </div>
    </div>
    <div id="fullLogContainer" class="log-terminal" style="height:420px">
      <div class="log-line"><span class="log-time">[SYSTEM]</span> <span class="log-msg">Loading daemon and kernel logs...</span></div>
    </div>
  </div>
</div>

<!-- Tab 7: Command Console -->
<div id="tab-console" class="hidden">
  <div class="card">
    <div class="card-header">
      <div>
        <h2 style="font-size:1.15rem; font-weight:700">💻 Unified Command Console</h2>
        <p style="font-size:0.85rem; color:var(--text-muted)">
          Execute WireNet operational tasks, kernel inspection (conntrack/nftables), and Minecraft RCON console commands.
        </p>
      </div>
      <span id="cmdExecTimer" style="font-family:var(--mono); font-size:0.8rem; color:var(--text-muted)">Ready</span>
    </div>

    <!-- Quick action buttons -->
    <div class="quick-ops-bar">
      <button class="btn btn-secondary" style="font-size:0.75rem" onclick="runQuickCmd('wirenet doctor')">🩺 wirenet doctor</button>
      <button class="btn btn-secondary" style="font-size:0.75rem" onclick="runQuickCmd('wirenet status')">📊 wirenet status</button>
      <button class="btn btn-secondary" style="font-size:0.75rem" onclick="runQuickCmd('wirenet apply')">🔄 wirenet apply</button>
      <button class="btn btn-secondary" style="font-size:0.75rem" onclick="runQuickCmd('conntrack -L')">🛡️ conntrack -L</button>
      <button class="btn btn-secondary" style="font-size:0.75rem" onclick="runQuickCmd('ip rule show; ip route show table 100')">🔀 Policy Routes</button>
      <button class="btn btn-secondary" style="font-size:0.75rem" onclick="runQuickCmd('nft list ruleset')">📋 nftables Rules</button>
      <button class="btn btn-secondary" style="font-size:0.75rem" onclick="runSniffCmd()">🌐 Sniff wg0 Packets</button>
    </div>

    <div id="consoleOutput" class="console-box">WireNet Interactive Terminal [Version 2.0.0]&#10;Type 'help' or click any quick command above to begin.&#10;For Minecraft server commands, prefix with 'mc:' (e.g. 'mc: list').&#10;&#10;wirenet@hub:~$ </div>

    <div class="console-prompt-bar">
      <input id="consoleInput" class="console-input" placeholder="Type command (e.g. wirenet doctor, conntrack -L, mc: list)..." onkeydown="handleConsoleKey(event)">
      <button class="btn" onclick="executeConsoleInput()">Run</button>
      <button class="btn btn-secondary" onclick="clearConsole()">Clear</button>
    </div>
  </div>
</div>

<script>
let token = localStorage.getItem('wirenet_dashboard_token') || '';
let isPaused = false;
let cmdHistory = [];
let cmdHistIdx = -1;

if (token) {
  document.getElementById('authToken').value = token;
}

function authHeaders() {
  return {
    'Authorization': 'Bearer ' + (document.getElementById('authToken').value.trim() || token),
    'Content-Type': 'application/json'
  };
}

function saveAndConnect() {
  token = document.getElementById('authToken').value.trim();
  localStorage.setItem('wirenet_dashboard_token', token);
  showAlert('Token saved. Connecting to telemetry...', 'success');
  poll();
  fetchLogs();
}

function showAlert(msg, type='error') {
  const box = document.getElementById('alertBox');
  box.className = 'alert ' + (type === 'success' ? 'alert-success' : 'alert-error');
  box.textContent = msg;
  box.style.display = 'block';
  setTimeout(() => { box.style.display = 'none'; }, 4000);
}

function switchTab(tab) {
  ['monitor', 'minecraft', 'mappings', 'nodes', 'security', 'logs', 'console'].forEach(t => {
    const el = document.getElementById('tab-' + t);
    if (el) el.classList.add('hidden');
  });
  const target = document.getElementById('tab-' + tab);
  if (target) target.classList.remove('hidden');

  document.querySelectorAll('nav .tab-btn').forEach(btn => {
    btn.classList.remove('active');
  });
  if (event && event.target) {
    event.target.classList.add('active');
  }

  if (tab === 'minecraft') refreshMinecraftServers();
  if (tab === 'logs') fetchLogs();
}

function togglePause() {
  isPaused = !isPaused;
  document.getElementById('pauseBtn').textContent = isPaused ? 'Resume Stream' : 'Pause Stream';
}

function clearLogs() {
  document.getElementById('eventLogContainer').innerHTML = '';
}

function formatBytes(bytes) {
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function formatSeconds(secs) {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  return `${String(h).padStart(2,'0')}:${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`;
}

async function poll() {
  const curToken = document.getElementById('authToken').value.trim();
  if (!curToken) {
    document.getElementById('connStatusPill').className = 'status-pill offline';
    document.getElementById('connStatusText').textContent = 'Token Required';
    return;
  }

  try {
    const tStart = performance.now();
    const res = await fetch('/api/telemetry', { headers: authHeaders() });
    const latency = Math.round(performance.now() - tStart);

    if (res.status === 401) {
      document.getElementById('connStatusPill').className = 'status-pill offline';
      document.getElementById('connStatusText').textContent = 'Unauthorized';
      return;
    }
    if (!res.ok) throw new Error('HTTP ' + res.status);

    const data = await res.json();
    document.getElementById('connStatusPill').className = 'status-pill online';
    document.getElementById('connStatusText').textContent = `Connected (${latency}ms)`;

    renderTelemetry(data);
    await loadState();
  } catch (err) {
    document.getElementById('connStatusPill').className = 'status-pill offline';
    document.getElementById('connStatusText').textContent = 'Offline / Error';
  }
}

function renderTelemetry(data) {
  // 1. Link status
  const linkPill = document.getElementById('linkPill');
  linkPill.className = 'status-pill ' + (data.status === 'ONLINE' ? 'online' : 'offline');
  linkPill.innerHTML = `<span class="dot"></span> ${data.status === 'ONLINE' ? 'LIVE KERNEL LINK' : data.status}`;
  document.getElementById('metricInterface').textContent = data.interface.name + (data.interface.exists ? '' : ' (OFFLINE)');
  document.getElementById('metricUptime').textContent = 'Uptime: ' + formatSeconds(data.uptime_seconds);

  // 2. Traffic
  document.getElementById('livePps').textContent = `${data.interface.current_pps} pkts/s`;
  document.getElementById('metricTotalPackets').textContent = data.interface.total_packets.toLocaleString();
  document.getElementById('metricBytes').textContent = `RX: ${formatBytes(data.interface.rx_bytes)} │ TX: ${formatBytes(data.interface.tx_bytes)}`;

  // Sparkline
  if (data.traffic_history && data.traffic_history.length > 0) {
    const maxVal = Math.max(...data.traffic_history, 10);
    const w = 300, h = 45;
    const pts = data.traffic_history.map((val, idx) => {
      const x = (idx / (data.traffic_history.length - 1)) * w;
      const y = h - ((val / maxVal) * (h - 6));
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    }).join(' ');
    document.getElementById('sparklinePoly').setAttribute('points', pts);
  }

  // 3. Tunnel Load
  document.getElementById('loadPercentText').textContent = `${data.interface.load_percentage}%`;
  document.getElementById('loadProgressBar').style.width = `${data.interface.load_percentage}%`;
  document.getElementById('activePlayersCount').textContent = `${data.active_players.length} Players`;

  if (data.peers && data.peers.length > 0) {
    const p = data.peers[0];
    document.getElementById('peerHandshake').textContent = `Peer: ${p.endpoint || 'Connected'} (${p.latest_handshake_ago})`;
  } else {
    document.getElementById('peerHandshake').textContent = 'No WireGuard peers connected';
  }

  // 4. Protection
  document.getElementById('metricShield').textContent = data.protection.shield_mode.split(' ')[0];
  document.getElementById('metricConntrack').textContent = `Conntrack: ${data.protection.conntrack_count.toLocaleString()} states`;

  // 5. Active Players Table with DROP and BAN actions
  const tbody = document.getElementById('playerTableBody');
  if (!data.active_players || data.active_players.length === 0) {
    tbody.innerHTML = `<tr><td colspan="7" style="text-align:center; color:var(--text-muted); padding:1.5rem">
      ● Waiting for player connection packets on ${data.interface.name}... (0 active flows)
    </td></tr>`;
  } else {
    tbody.innerHTML = data.active_players.map(p => `
      <tr>
        <td class="mono" style="color:var(--green); font-weight:700">${p.client_ip}</td>
        <td class="mono">${p.client_port}</td>
        <td class="mono" style="color:var(--cyan); font-weight:600">${p.game_port}</td>
        <td><span class="tag-proto">${p.protocol}</span></td>
        <td><span class="tag-state">${p.state}</span></td>
        <td class="mono" style="color:var(--text-muted)">${formatBytes(p.bytes)}</td>
        <td>
          <div style="display:flex; gap:0.4rem">
            <button class="btn btn-danger" style="font-size:0.75rem; padding:0.2rem 0.6rem"
              onclick="dropFlow('${p.client_ip}', ${p.client_port}, '${p.protocol}')">
              ⚡ Drop
            </button>
            <button class="btn btn-secondary" style="font-size:0.75rem; padding:0.2rem 0.6rem"
              onclick="prefillBan('${p.client_ip}')">
              🛡️ Ban
            </button>
          </div>
        </td>
      </tr>
    `).join('');
  }

  // 6. Packet Event Logs
  if (!isPaused && data.packet_events && data.packet_events.length > 0) {
    const logBox = document.getElementById('eventLogContainer');
    logBox.innerHTML = data.packet_events.map(ev => {
      let tagClass = 'log-tag-traffic';
      if (ev.event_type === 'CONNECTED') tagClass = 'log-tag-connected';
      if (ev.event_type === 'DISCONNECTED') tagClass = 'log-tag-disconnected';
      return `<div class="log-line">
        <span class="log-time">[${ev.timestamp}]</span>
        <span class="${tagClass}">[${ev.event_type}]</span>
        <span class="log-msg">${ev.message}</span>
      </div>`;
    }).join('');
    logBox.scrollTop = logBox.scrollHeight;
  }
}

async function dropFlow(ip, port, proto) {
  if (!confirm(`Are you sure you want to terminate kernel connection flow for ${ip}:${port} (${proto})?`)) return;
  try {
    const res = await fetch('/api/connections/drop', {
      method: 'POST',
      headers: authHeaders(),
      body: JSON.stringify({ client_ip: ip, client_port: port, protocol: proto })
    });
    if (res.ok) {
      showAlert(`Terminated connection flow ${ip}:${port}`, 'success');
      poll();
    } else {
      showAlert('Failed to drop connection: ' + (await res.text()));
    }
  } catch (e) {
    showAlert('Error dropping connection: ' + e.message);
  }
}

function prefillBan(ip) {
  switchTab('security');
  document.getElementById('b_ip').value = ip;
  document.getElementById('b_reason').value = 'Terminated from live monitor';
  document.getElementById('b_ip').focus();
}

async function loadState() {
  try {
    const res = await fetch('/api/state', { headers: authHeaders() });
    if (!res.ok) return;
    const state = await res.json();

    // Render Mappings
    const mapBody = document.getElementById('mappingsTableBody');
    if (!state.mappings || state.mappings.length === 0) {
      mapBody.innerHTML = '<tr><td colspan="7" style="text-align:center; color:var(--text-muted)">No ingress mappings configured.</td></tr>';
    } else {
      mapBody.innerHTML = state.mappings.map(m => `
        <tr>
          <td class="mono">${m.id}</td>
          <td class="mono">${m.server_id}</td>
          <td class="mono" style="color:var(--cyan)">${m.public_ip}:${m.public_port}</td>
          <td class="mono">${m.backend_port}</td>
          <td><span class="tag-proto">${m.protocol.toUpperCase()}</span></td>
          <td><span class="tag-state" style="${m.enabled ? '' : 'background:rgba(239,68,68,0.2); color:var(--rose)'}">
            ${m.enabled ? 'ACTIVE' : 'DISABLED'}
          </span></td>
          <td>
            <button class="btn btn-secondary" style="font-size:0.75rem; padding:0.2rem 0.6rem"
              onclick="toggleMapping('${m.id}', ${!m.enabled})">
              ${m.enabled ? 'Disable' : 'Enable'}
            </button>
          </td>
        </tr>
      `).join('');
    }

    // Render Nodes
    const nodeBody = document.getElementById('nodesTableBody');
    if (!state.nodes || state.nodes.length === 0) {
      nodeBody.innerHTML = '<tr><td colspan="4" style="text-align:center; color:var(--text-muted)">No nodes enrolled.</td></tr>';
    } else {
      nodeBody.innerHTML = state.nodes.map(n => `
        <tr>
          <td class="mono">${n.id}</td>
          <td>${n.name}</td>
          <td class="mono" style="color:var(--cyan)">${n.tunnel_ip}</td>
          <td class="mono" style="font-size:0.75rem">${n.public_key}</td>
        </tr>
      `).join('');
    }

    // Render Servers
    const srvBody = document.getElementById('serversTableBody');
    if (!state.servers || state.servers.length === 0) {
      srvBody.innerHTML = '<tr><td colspan="4" style="text-align:center; color:var(--text-muted)">No servers registered.</td></tr>';
    } else {
      srvBody.innerHTML = state.servers.map(s => `
        <tr>
          <td class="mono">${s.id}</td>
          <td class="mono">${s.node_id}</td>
          <td class="mono">${s.customer_id}</td>
          <td class="mono" style="font-size:0.75rem">${s.pterodactyl_id}</td>
        </tr>
      `).join('');
    }

    // Render Bans
    const banBody = document.getElementById('bansTableBody');
    if (!state.bans || state.bans.length === 0) {
      banBody.innerHTML = '<tr><td colspan="6" style="text-align:center; color:var(--text-muted)">No active bans.</td></tr>';
    } else {
      banBody.innerHTML = state.bans.map(b => `
        <tr>
          <td class="mono">${b.id}</td>
          <td class="mono" style="color:var(--rose)">${b.ip}</td>
          <td>${b.reason}</td>
          <td class="mono">${b.mapping_id || '-'}</td>
          <td class="mono" style="font-size:0.75rem">${b.created_at}</td>
          <td>
            <button class="btn btn-danger" style="font-size:0.75rem; padding:0.2rem 0.6rem"
              onclick="deleteBan(${b.id})">
              Delete
            </button>
          </td>
        </tr>
      `).join('');
    }
  } catch (err) {}
}

async function submitMapping() {
  const data = {
    id: document.getElementById('m_id').value.trim(),
    server_id: document.getElementById('m_server').value.trim(),
    public_ip: document.getElementById('m_ip').value.trim(),
    public_port: parseInt(document.getElementById('m_port').value.trim(), 10),
    backend_port: parseInt(document.getElementById('m_bport').value.trim(), 10),
    protocol: document.getElementById('m_proto').value
  };
  if (!data.id || !data.public_ip || isNaN(data.public_port)) {
    showAlert('Please fill in all mapping fields');
    return;
  }
  const r = await fetch('/api/mappings', { method: 'POST', headers: authHeaders(), body: JSON.stringify(data) });
  if (r.ok) {
    showAlert('Mapping reserved successfully', 'success');
    poll();
  } else {
    showAlert(await r.text());
  }
}

async function toggleMapping(id, enable) {
  const url = `/api/mappings/${id}/${enable ? 'enable' : 'disable'}`;
  const r = await fetch(url, { method: 'POST', headers: authHeaders() });
  if (r.ok) poll();
  else showAlert(await r.text());
}

async function submitNode() {
  const data = {
    id: document.getElementById('n_id').value.trim(),
    name: document.getElementById('n_name').value.trim(),
    tunnel_ip: document.getElementById('n_ip').value.trim(),
    public_key: document.getElementById('n_key').value.trim()
  };
  const r = await fetch('/api/nodes', { method: 'POST', headers: authHeaders(), body: JSON.stringify(data) });
  if (r.ok) {
    showAlert('Node enrolled successfully', 'success');
    poll();
  } else showAlert(await r.text());
}

async function submitServer() {
  const data = {
    id: document.getElementById('s_id').value.trim(),
    node_id: document.getElementById('s_node').value.trim(),
    customer_id: document.getElementById('s_cust').value.trim(),
    pterodactyl_id: document.getElementById('s_ptero').value.trim()
  };
  const r = await fetch('/api/servers', { method: 'POST', headers: authHeaders(), body: JSON.stringify(data) });
  if (r.ok) {
    showAlert('Server registered successfully', 'success');
    poll();
  } else showAlert(await r.text());
}

async function submitBan() {
  const exp = document.getElementById('b_exp').value.trim();
  const data = {
    ip: document.getElementById('b_ip').value.trim(),
    reason: document.getElementById('b_reason').value.trim(),
    mapping_id: document.getElementById('b_mapping').value.trim() || null,
    expires_in_secs: exp ? parseInt(exp, 10) : null
  };
  const r = await fetch('/api/bans', { method: 'POST', headers: authHeaders(), body: JSON.stringify(data) });
  if (r.ok) {
    showAlert('IP ban recorded', 'success');
    poll();
  } else showAlert(await r.text());
}

async function deleteBan(id) {
  const r = await fetch(`/api/bans/${id}/delete`, { method: 'POST', headers: authHeaders() });
  if (r.ok) poll();
  else showAlert(await r.text());
}

/* Minecraft Servers & SLP Functions */
async function refreshMinecraftServers() {
  const grid = document.getElementById('mcCardsGrid');
  try {
    const res = await fetch('/api/state', { headers: authHeaders() });
    if (!res.ok) return;
    const state = await res.json();
    if (!state.mappings || state.mappings.length === 0) {
      grid.innerHTML = '<div style="color:var(--text-muted); padding:1rem">No configured server mappings. Add mappings in the Port Mappings tab.</div>';
      return;
    }

    grid.innerHTML = '<div style="color:var(--text-muted); padding:1rem">Probing Minecraft servers with authentic SLP packets...</div>';
    let html = '';

    for (const m of state.mappings) {
      if (!m.enabled) continue;
      const statusRes = await fetch(`/api/minecraft/status?host=127.0.0.1&port=${m.backend_port}`, { headers: authHeaders() });
      const st = statusRes.ok ? await statusRes.json() : { online: false, version_name: 'Unknown', motd: 'Offline' };

      const onlineBadge = st.online 
        ? `<span class="status-pill online"><span class="dot"></span> ONLINE</span>`
        : `<span class="status-pill offline"><span class="dot"></span> OFFLINE</span>`;

      html += `
        <div class="mc-card">
          <div class="mc-card-header">
            <div>
              <div class="mc-title">${m.server_id || m.id}</div>
              <div style="font-size:0.8rem; color:var(--text-muted); font-family:var(--mono)">Port ${m.backend_port} (${m.protocol.toUpperCase()})</div>
            </div>
            ${onlineBadge}
          </div>

          <div class="mc-motd-box">
            ${st.online ? renderColoredMotd(st.motd || 'A Minecraft Server') : '<span style="color:#64748b">Server not responding to SLP ping</span>'}
          </div>

          <div class="mc-stat-row">
            <span>Edition: <strong style="color:#fff">${st.edition || (m.protocol === 'udp' ? 'Bedrock' : 'Java')}</strong></span>
            <span>Version: <strong style="color:var(--cyan)">${st.version_name || '-'}</strong></span>
          </div>

          <div class="mc-stat-row">
            <span>Players: <strong style="color:var(--green)">${st.online_players || 0} / ${st.max_players || 0}</strong></span>
            <span>Ping: <strong style="color:var(--amber)">${st.latency_ms ? st.latency_ms + 'ms' : '-'}</strong></span>
          </div>

          <!-- RCON console for this server -->
          <div style="margin-top:0.5rem; border-top:1px solid var(--card-border); padding-top:0.5rem">
            <div style="font-size:0.75rem; color:var(--text-muted); margin-bottom:0.25rem">Quick RCON Console:</div>
            <div class="mc-rcon-input-group">
              <input id="rcon_cmd_${m.id}" class="form-input" style="flex:1" placeholder="RCON Command (e.g. list, tps, say Hi)...">
              <input id="rcon_pass_${m.id}" class="form-input" type="password" style="width:110px" placeholder="Password">
              <button class="btn" style="font-size:0.75rem" onclick="sendServerRcon('${m.id}', ${m.backend_port})">Send</button>
            </div>
            <div id="rcon_out_${m.id}" style="display:none; margin-top:0.4rem; font-size:0.75rem; font-family:var(--mono); background:#020408; padding:0.4rem; border-radius:4px; color:#38bdf8"></div>
          </div>
        </div>
      `;
    }

    grid.innerHTML = html || '<div style="color:var(--text-muted); padding:1rem">No enabled mappings found.</div>';
  } catch (e) {
    grid.innerHTML = `<div style="color:var(--rose); padding:1rem">Failed probing servers: ${e.message}</div>`;
  }
}

async function sendServerRcon(mapId, port) {
  const cmd = document.getElementById(`rcon_cmd_${mapId}`).value.trim();
  const pass = document.getElementById(`rcon_pass_${mapId}`).value.trim();
  const outBox = document.getElementById(`rcon_out_${mapId}`);
  if (!cmd) return;

  outBox.style.display = 'block';
  outBox.textContent = 'Executing...';

  try {
    const res = await fetch('/api/minecraft/rcon', {
      method: 'POST',
      headers: authHeaders(),
      body: JSON.stringify({ host: '127.0.0.1', port: port, password: pass, command: cmd })
    });
    const data = await res.json();
    outBox.textContent = data.output || (data.success ? 'Success (no output)' : 'Failed');
    outBox.style.color = data.success ? '#38bdf8' : '#ef4444';
  } catch (e) {
    outBox.textContent = 'Error: ' + e.message;
    outBox.style.color = '#ef4444';
  }
}

async function runManualProbe() {
  const host = document.getElementById('probe_host').value.trim() || '127.0.0.1';
  const port = parseInt(document.getElementById('probe_port').value.trim() || '25565', 10);
  const box = document.getElementById('probeResultBox');
  box.style.display = 'block';
  box.innerHTML = 'Sending real SLP / RakNet ping...';

  try {
    const res = await fetch(`/api/minecraft/status?host=${encodeURIComponent(host)}&port=${port}`, { headers: authHeaders() });
    const data = await res.json();
    if (data.online) {
      box.innerHTML = `
        <div style="color:var(--green); font-weight:700">✓ Server Online (${data.edition}) — Latency: ${data.latency_ms}ms</div>
        <div style="margin-top:0.3rem">Version: <strong>${data.version_name}</strong> (Protocol ${data.protocol_version})</div>
        <div>Players: <strong>${data.online_players} / ${data.max_players}</strong></div>
        <div style="margin-top:0.4rem; padding:0.5rem; background:#020408; border-radius:4px">${renderColoredMotd(data.motd || 'A Minecraft Server')}</div>
      `;
    } else {
      box.innerHTML = `<span style="color:var(--rose)">✗ Server unreachable at ${host}:${port}</span>`;
    }
  } catch (e) {
    box.innerHTML = `<span style="color:var(--rose)">Probe error: ${e.message}</span>`;
  }
}

function renderColoredMotd(motd) {
  if (!motd) return '';
  const colors = {
    '0': '#000000', '1': '#0000aa', '2': '#00aa00', '3': '#00aaaa',
    '4': '#aa0000', '5': '#aa00aa', '6': '#ffaa00', '7': '#aaaaaa',
    '8': '#555555', '9': '#5555ff', 'a': '#55ff55', 'b': '#55ffff',
    'c': '#ff5555', 'd': '#ff55ff', 'e': '#ffff55', 'f': '#ffffff'
  };
  let curColor = '#f8fafc';
  let isBold = false;
  let res = '';
  let i = 0;
  while (i < motd.length) {
    if ((motd[i] === '§' || motd[i] === '&') && i + 1 < motd.length) {
      const code = motd[i+1].toLowerCase();
      if (colors[code]) {
        curColor = colors[code];
        isBold = false;
      } else if (code === 'l') {
        isBold = true;
      } else if (code === 'r') {
        curColor = '#f8fafc';
        isBold = false;
      }
      i += 2;
    } else {
      const char = motd[i] === '\n' ? '<br>' : (motd[i] === ' ' ? '&nbsp;' : motd[i]);
      res += `<span style="color:${curColor}; font-weight:${isBold ? '700' : '400'}">${char}</span>`;
      i++;
    }
  }
  return res;
}

/* Real-Time Logs Viewer */
async function fetchLogs() {
  const container = document.getElementById('fullLogContainer');
  const lvl = document.getElementById('logLevelSelect').value;
  try {
    const res = await fetch(`/api/logs?level=${lvl}&limit=150`, { headers: authHeaders() });
    if (!res.ok) return;
    const logs = await res.json();
    if (!logs || logs.length === 0) {
      container.innerHTML = '<div class="log-line"><span class="log-msg" style="color:var(--text-muted)">No logs recorded yet.</span></div>';
      return;
    }
    container.innerHTML = logs.map(l => {
      let lvlClass = 'log-lvl-info';
      if (l.level === 'WARN') lvlClass = 'log-lvl-warn';
      if (l.level === 'ERROR') lvlClass = 'log-lvl-error';
      if (l.level === 'SECURITY') lvlClass = 'log-lvl-security';
      if (l.level === 'PACKET') lvlClass = 'log-lvl-packet';
      return `<div class="log-line">
        <span class="log-time">[${l.timestamp}]</span>
        <span class="log-source">[${l.source}]</span>
        <span class="${lvlClass}">[${l.level}]</span>
        <span class="log-msg">${l.message}</span>
      </div>`;
    }).join('');
    container.scrollTop = container.scrollHeight;
  } catch (e) {}
}

function downloadLogs() {
  const text = document.getElementById('fullLogContainer').innerText;
  const blob = new Blob([text], { type: 'text/plain' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = `wirenet_logs_${Date.now()}.log`;
  a.click();
}

/* Command Console */
function appendConsole(text) {
  const box = document.getElementById('consoleOutput');
  box.textContent += text + '\n';
  box.scrollTop = box.scrollHeight;
}

function clearConsole() {
  document.getElementById('consoleOutput').textContent = 'wirenet@hub:~$ ';
}

function runQuickCmd(cmd) {
  document.getElementById('consoleInput').value = cmd;
  executeConsoleInput();
}

async function runSniffCmd() {
  appendConsole(`wirenet@hub:~$ tcpdump -c 20 -nn -i wg0`);
  try {
    const t0 = performance.now();
    const res = await fetch('/api/packets/sniff', { headers: authHeaders() });
    const dt = Math.round(performance.now() - t0);
    if (!res.ok) throw new Error(await res.text());
    const packets = await res.json();
    if (packets.length === 0) {
      appendConsole('No packets captured on wg0 during sample window.\nwirenet@hub:~$ ');
    } else {
      appendConsole(packets.join('\n') + `\n[Sample captured in ${dt}ms]\nwirenet@hub:~$ `);
    }
  } catch (e) {
    appendConsole(`Sniff error: ${e.message}\nwirenet@hub:~$ `);
  }
}

async function executeConsoleInput() {
  const input = document.getElementById('consoleInput');
  const cmd = input.value.trim();
  if (!cmd) return;

  cmdHistory.push(cmd);
  cmdHistIdx = cmdHistory.length;
  input.value = '';

  appendConsole(`wirenet@hub:~$ ${cmd}`);
  document.getElementById('cmdExecTimer').textContent = 'Running...';

  try {
    const t0 = performance.now();
    const res = await fetch('/api/commands/exec', {
      method: 'POST',
      headers: authHeaders(),
      body: JSON.stringify({ command: cmd })
    });
    const dt = Math.round(performance.now() - t0);
    document.getElementById('cmdExecTimer').textContent = `Done in ${dt}ms`;

    if (!res.ok) throw new Error(await res.text());
    const data = await res.json();
    appendConsole(data.output + `\n[Finished in ${data.duration_ms}ms]\nwirenet@hub:~$ `);
  } catch (e) {
    document.getElementById('cmdExecTimer').textContent = 'Error';
    appendConsole(`Command failed: ${e.message}\nwirenet@hub:~$ `);
  }
}

function handleConsoleKey(e) {
  if (e.key === 'Enter') {
    executeConsoleInput();
  } else if (e.key === 'ArrowUp') {
    if (cmdHistIdx > 0) {
      cmdHistIdx--;
      document.getElementById('consoleInput').value = cmdHistory[cmdHistIdx];
    }
  } else if (e.key === 'ArrowDown') {
    if (cmdHistIdx < cmdHistory.length - 1) {
      cmdHistIdx++;
      document.getElementById('consoleInput').value = cmdHistory[cmdHistIdx];
    } else {
      cmdHistIdx = cmdHistory.length;
      document.getElementById('consoleInput').value = '';
    }
  }
}

// Initial polls
poll();
setInterval(poll, 1000);
</script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_listen_loopback() {
        assert!(validate_listen("127.0.0.1:8080".parse().unwrap(), false).is_ok());
        assert!(validate_listen("[::1]:8080".parse().unwrap(), false).is_ok());
        assert!(validate_listen("0.0.0.0:8080".parse().unwrap(), false).is_err());
        assert!(validate_listen("192.168.1.50:8080".parse().unwrap(), false).is_err());
        assert!(validate_listen("0.0.0.0:8080".parse().unwrap(), true).is_ok());
    }

    #[test]
    fn test_token_creation_and_load() {
        let temp_dir = std::env::temp_dir().join(format!("wirenet_test_{}", rand::random::<u32>()));
        let token_path = temp_dir.join("test.token");
        let token = create_token(&token_path).unwrap();
        assert_eq!(token.len(), 64);
        assert!(create_token(&token_path).is_err()); // Refuses to overwrite
        let loaded = load_token(&token_path).unwrap();
        assert_eq!(token, loaded);
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
