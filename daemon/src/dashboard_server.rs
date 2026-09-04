use crate::controller::{BanInput, MappingInput, NodeInput, ServerInput, StateView, Store};
use anyhow::{bail, Context, Result};
use axum::{
    extract::{Path as AxumPath, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use rand::RngCore;
use std::{net::SocketAddr, path::Path, process::Command};
use subtle::ConstantTimeEq;

#[derive(Clone)]
struct AppState {
    store: Store,
    token: String,
}

pub async fn serve(store: Store, listen: SocketAddr, token: String) -> Result<()> {
    let app = Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/state", get(state))
        .route("/api/nodes", post(create_node))
        .route("/api/servers", post(create_server))
        .route("/api/mappings", post(create_mapping))
        .route("/api/mappings/:id/enable", post(enable_mapping))
        .route("/api/mappings/:id/disable", post(disable_mapping))
        .route("/api/bans", post(create_ban))
        .route("/api/bans/:id/delete", post(delete_ban))
        .with_state(AppState { store, token });
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

const INDEX: &str = r#"<!doctype html><title>WireNet dashboard</title><style>body{font:15px system-ui;max-width:1100px;margin:2rem auto;padding:0 1rem}input,select,button{padding:.45rem;margin:.15rem}fieldset{margin:1rem 0}table{border-collapse:collapse;width:100%}td,th{border:1px solid #bbb;padding:.4rem;text-align:left}.error{color:#b00}</style><h1>WireNet dashboard</h1><p>Optional management UI. It edits desired state only; apply is intentionally separate.</p><label>Dashboard token <input id=token type=password><button onclick=load()>Connect</button></label><p id=message class=error></p><fieldset><legend>Add node</legend><input id=nid placeholder="node id"><input id=nname placeholder="name"><input id=nip placeholder="10.100.0.2"><input id=nkey placeholder="WireGuard public key"><button onclick="post('/api/nodes',{id:nid.value,name:nname.value,tunnel_ip:nip.value,public_key:nkey.value})">Add</button></fieldset><fieldset><legend>Add server</legend><input id=sid placeholder="server id"><input id=snode placeholder="node id"><input id=scustomer placeholder="customer id"><input id=sptero placeholder="Pterodactyl UUID"><button onclick="post('/api/servers',{id:sid.value,node_id:snode.value,customer_id:scustomer.value,pterodactyl_id:sptero.value})">Add</button></fieldset><fieldset><legend>Add mapping</legend><input id=mid placeholder="mapping id"><input id=mserver placeholder="server id"><input id=mip placeholder="public IPv4"><input id=mport type=number placeholder="public port"><input id=mbport type=number placeholder="backend port"><select id=mproto><option>tcp</option><option>udp</option></select><button onclick="post('/api/mappings',{id:mid.value,server_id:mserver.value,public_ip:mip.value,public_port:+mport.value,backend_port:+mbport.value,protocol:mproto.value})">Reserve</button></fieldset><h2>Desired state</h2><pre id=state>Connect to load state.</pre><script>const msg=document.querySelector('#message');function h(){return {Authorization:'Bearer '+token.value,'Content-Type':'application/json'}}async function load(){let r=await fetch('/api/state',{headers:h()});if(!r.ok){msg.textContent=await r.text();return}state.textContent=JSON.stringify(await r.json(),null,2);msg.textContent=''}async function post(url,data){let r=await fetch(url,{method:'POST',headers:h(),body:JSON.stringify(data)});if(!r.ok){msg.textContent=await r.text();return}await load()}</script>"#;

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
