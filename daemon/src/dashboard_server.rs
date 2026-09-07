use crate::controller::{BanInput, MappingInput, NodeInput, ServerInput, StateView, Store};
use crate::net::telemetry::{SystemTelemetry, TelemetryCollector};
use anyhow::{bail, Context, Result};
use axum::{
    extract::{Path as AxumPath, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use rand::RngCore;
use std::{net::SocketAddr, path::Path, process::Command, sync::Arc};
use subtle::ConstantTimeEq;
use tokio::sync::Mutex;

#[derive(Clone)]
struct AppState {
    store: Store,
    token: String,
    telemetry: Arc<Mutex<TelemetryCollector>>,
}

pub async fn serve(store: Store, listen: SocketAddr, token: String) -> Result<()> {
    let telemetry = Arc::new(Mutex::new(TelemetryCollector::new("wg0")));
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
        .with_state(AppState {
            store,
            token,
            telemetry,
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
<title>WireNet — Real-Time Control & Telemetry Center</title>
<style>
:root {
  --bg: #090d16;
  --surface: #0f172a;
  --card: #141e33;
  --card-border: #1e293b;
  --text: #f8fafc;
  --text-muted: #94a3b8;
  --cyan: #00f0ff;
  --green: #10b981;
  --amber: #f59e0b;
  --rose: #ef4444;
  --mono: 'JetBrains Mono', 'Fira Code', ui-monospace, monospace;
}
* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  background: var(--bg);
  color: var(--text);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
  line-height: 1.5;
  padding: 1.5rem;
  max-width: 1400px;
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
  margin-bottom: 1.5rem;
  box-shadow: 0 4px 20px rgba(0,0,0,0.3);
}
.brand {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}
.brand h1 {
  font-size: 1.4rem;
  font-weight: 700;
  color: #fff;
  letter-spacing: -0.5px;
}
.badge-version {
  background: rgba(0,240,255,0.15);
  color: var(--cyan);
  font-size: 0.75rem;
  padding: 0.2rem 0.6rem;
  border-radius: 20px;
  font-family: var(--mono);
}
.auth-bar {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.input-token {
  background: #060911;
  border: 1px solid var(--card-border);
  color: #fff;
  padding: 0.45rem 0.75rem;
  border-radius: 6px;
  font-family: var(--mono);
  font-size: 0.85rem;
  width: 280px;
}
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
}
.btn:hover { filter: brightness(1.15); }
.btn-secondary {
  background: var(--card);
  color: var(--text);
  border: 1px solid var(--card-border);
}
.btn-danger {
  background: rgba(239, 68, 68, 0.2);
  color: var(--rose);
  border: 1px solid var(--rose);
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
  gap: 0.5rem;
  margin-bottom: 1.5rem;
  border-bottom: 1px solid var(--card-border);
  padding-bottom: 0.5rem;
}
.tab-btn {
  background: transparent;
  color: var(--text-muted);
  border: none;
  font-size: 0.95rem;
  font-weight: 600;
  padding: 0.5rem 1rem;
  cursor: pointer;
  border-radius: 6px;
  transition: all 0.2s;
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
/* Overview Metric Cards */
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
  box-shadow: 0 4px 15px rgba(0,0,0,0.2);
}
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.75rem;
}
.card-title {
  font-size: 0.85rem;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--text-muted);
  letter-spacing: 0.5px;
}
.card-value {
  font-size: 1.8rem;
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
/* Live Traffic Sparkline & Gauge */
.sparkline-container {
  margin-top: 0.75rem;
  height: 50px;
  display: flex;
  align-items: flex-end;
}
.progress-bar-bg {
  background: #060911;
  border-radius: 6px;
  height: 10px;
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
  background: #090f1e;
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
tr:hover td {
  background: rgba(255,255,255,0.02);
}
.mono {
  font-family: var(--mono);
}
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
/* Event Log Terminal */
.log-terminal {
  background: #050811;
  border: 1px solid var(--card-border);
  border-radius: 8px;
  padding: 1rem;
  height: 240px;
  overflow-y: auto;
  font-family: var(--mono);
  font-size: 0.8rem;
  line-height: 1.6;
}
.log-line {
  display: flex;
  gap: 0.75rem;
  margin-bottom: 0.25rem;
}
.log-time { color: #64748b; }
.log-msg { color: #e2e8f0; }
.log-tag-connected { color: var(--green); font-weight: 600; }
.log-tag-traffic { color: var(--cyan); }
.log-tag-disconnected { color: var(--amber); }
/* Forms */
.form-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 0.75rem;
  margin-top: 1rem;
}
.form-input {
  background: #060911;
  border: 1px solid var(--card-border);
  color: #fff;
  padding: 0.5rem 0.75rem;
  border-radius: 6px;
  font-size: 0.85rem;
  font-family: var(--mono);
}
.form-input:focus {
  outline: none;
  border-color: var(--cyan);
}
.alert {
  padding: 0.75rem 1rem;
  border-radius: 8px;
  margin-bottom: 1rem;
  font-size: 0.85rem;
  display: none;
}
.alert-error {
  background: rgba(239,68,68,0.2);
  color: #fca5a5;
  border: 1px solid rgba(239,68,68,0.4);
}
.alert-success {
  background: rgba(16,185,129,0.2);
  color: #6ee7b7;
  border: 1px solid rgba(16,185,129,0.4);
}
.hidden { display: none; }
</style>
</head>
<body>

<header>
  <div class="brand">
    <span style="font-size: 1.6rem">🌐</span>
    <div>
      <div style="display:flex; align-items:center; gap:0.5rem">
        <h1>WireNet Control Center</h1>
        <span class="badge-version">Rust Kernel Engine</span>
      </div>
      <div style="font-size:0.8rem; color:var(--text-muted); margin-top:2px">
        Real-Time Zero-Proxy Ingress & Transparent Real IP Monitoring
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
  <button class="tab-btn" onclick="switchTab('mappings')">🔀 Port Mappings</button>
  <button class="tab-btn" onclick="switchTab('nodes')">🖥️ Nodes & Servers</button>
  <button class="tab-btn" onclick="switchTab('security')">🛡️ Anti-DDoS & IP Bans</button>
</nav>

<!-- Tab 1: Live Monitor -->
<div id="tab-monitor">
  <div class="metrics-grid">
    <!-- Metric 1: Link Status -->
    <div class="card">
      <div class="card-header">
        <span class="card-title">Kernel Link Status</span>
        <span id="linkPill" class="status-pill online"><span class="dot"></span> ONLINE</span>
      </div>
      <div id="metricInterface" class="card-value">wg0</div>
      <div id="metricUptime" class="card-subtext">Uptime: 00:00:00</div>
    </div>

    <!-- Metric 2: Live Traffic -->
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

    <!-- Metric 3: Tunnel Capacity -->
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

    <!-- Metric 4: Shield Status -->
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
          Authentic client source IPs parsed directly from Linux Kernel /proc/net/nf_conntrack & TCP sockets (No proxy masking)
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
        </tr>
      </thead>
      <tbody id="playerTableBody">
        <tr><td colspan="6" style="text-align:center; color:var(--text-muted)">Waiting for player connection packets on wg0...</td></tr>
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
    <div id="eventLogContainer" class="log-terminal">
      <div class="log-line"><span class="log-time">[SYSTEM]</span> <span class="log-msg">WireNet Real-Time Packet & IP Sniffer Active. Listening on tunnel fastpath...</span></div>
    </div>
  </div>
</div>

<!-- Tab 2: Port Mappings -->
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
        <option value="tcp">TCP</option>
        <option value="udp">UDP</option>
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

<!-- Tab 3: Nodes & Servers -->
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

<!-- Tab 4: Security & Bans -->
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

<script>
let token = localStorage.getItem('wirenet_dashboard_token') || '';
let isPaused = false;
let eventHistory = [];

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
}

function showAlert(msg, type='error') {
  const box = document.getElementById('alertBox');
  box.className = 'alert ' + (type === 'success' ? 'alert-success' : 'alert-error');
  box.textContent = msg;
  box.style.display = 'block';
  setTimeout(() => { box.style.display = 'none'; }, 4000);
}

function switchTab(tab) {
  ['monitor', 'mappings', 'nodes', 'security'].forEach(t => {
    document.getElementById('tab-' + t).classList.add('hidden');
  });
  document.getElementById('tab-' + tab).classList.remove('hidden');

  document.querySelectorAll('nav .tab-btn').forEach((btn, idx) => {
    btn.classList.remove('active');
  });
  event.target.classList.add('active');
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
      document.getElementById('connStatusText').textContent = 'Unauthorized (Check Token)';
      return;
    }
    if (!res.ok) {
      throw new Error('HTTP ' + res.status);
    }

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

  // Render Sparkline
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

  // 5. Active Players Table (100% Real IPs)
  const tbody = document.getElementById('playerTableBody');
  if (!data.active_players || data.active_players.length === 0) {
    tbody.innerHTML = `<tr><td colspan="6" style="text-align:center; color:var(--text-muted); padding:1.5rem">
      ● Waiting for player connection packets on ${data.interface.name}... (0 active players)
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

// Start live polling loop every 1000ms
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
        let temp_dir =
            std::env::temp_dir().join(format!("wirenet_test_{}", rand::random::<u32>()));
        let token_path = temp_dir.join("test.token");
        let token = create_token(&token_path).unwrap();
        assert_eq!(token.len(), 64);
        assert!(create_token(&token_path).is_err()); // Refuses to overwrite
        let loaded = load_token(&token_path).unwrap();
        assert_eq!(token, loaded);
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
