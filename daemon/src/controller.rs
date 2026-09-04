use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct Store(Arc<Mutex<Connection>>);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeInput {
    pub id: String,
    pub name: String,
    pub tunnel_ip: String,
    pub public_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ServerInput {
    pub id: String,
    pub node_id: String,
    pub customer_id: String,
    pub pterodactyl_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MappingInput {
    pub id: String,
    pub server_id: String,
    pub public_ip: String,
    pub public_port: u16,
    pub backend_port: u16,
    pub protocol: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BanInput {
    pub ip: String,
    pub mapping_id: Option<String>,
    pub reason: String,
    pub expires_at: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub tunnel_ip: String,
    pub public_key: String,
    pub status: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Server {
    pub id: String,
    pub node_id: String,
    pub customer_id: String,
    pub pterodactyl_id: String,
    pub state: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Mapping {
    pub id: String,
    pub server_id: String,
    pub node_id: String,
    pub public_ip: String,
    pub public_port: u16,
    pub backend_port: u16,
    pub protocol: String,
    pub enabled: bool,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Ban {
    pub id: i64,
    pub ip: String,
    pub mapping_id: Option<String>,
    pub reason: String,
    pub expires_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct StateView {
    pub nodes: Vec<Node>,
    pub servers: Vec<Server>,
    pub mappings: Vec<Mapping>,
    pub bans: Vec<Ban>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating database directory {}", parent.display()))?;
        }
        let connection = Connection::open(path)
            .with_context(|| format!("opening controller database {}", path.display()))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             CREATE TABLE IF NOT EXISTS nodes (
                 id TEXT PRIMARY KEY, name TEXT NOT NULL, tunnel_ip TEXT NOT NULL UNIQUE,
                 public_key TEXT NOT NULL UNIQUE, status TEXT NOT NULL, created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS servers (
                 id TEXT PRIMARY KEY, node_id TEXT NOT NULL REFERENCES nodes(id),
                 customer_id TEXT NOT NULL, pterodactyl_id TEXT NOT NULL UNIQUE,
                 state TEXT NOT NULL, created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS mappings (
                 id TEXT PRIMARY KEY, server_id TEXT NOT NULL REFERENCES servers(id),
                 public_ip TEXT NOT NULL, public_port INTEGER NOT NULL,
                 backend_port INTEGER NOT NULL, protocol TEXT NOT NULL,
                 enabled INTEGER NOT NULL, created_at INTEGER NOT NULL,
                 UNIQUE(public_ip, public_port, protocol)
             );
             CREATE TABLE IF NOT EXISTS bans (
                 id INTEGER PRIMARY KEY AUTOINCREMENT, ip TEXT NOT NULL,
                 mapping_id TEXT REFERENCES mappings(id), reason TEXT NOT NULL,
                 expires_at INTEGER, created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS audit_events (
                 id INTEGER PRIMARY KEY AUTOINCREMENT, actor TEXT NOT NULL, action TEXT NOT NULL,
                 target TEXT NOT NULL, detail TEXT NOT NULL, created_at INTEGER NOT NULL
             );",
        )?;
        Ok(Self(Arc::new(Mutex::new(connection))))
    }

    pub fn add_node(&self, input: NodeInput, actor: &str) -> Result<()> {
        validate_id(&input.id)?;
        validate_tunnel_ip(&input.tunnel_ip)?;
        if input.name.trim().is_empty() || input.public_key.trim().len() < 32 {
            bail!("node name and WireGuard public key are required");
        }
        let now = now();
        let db = self.db()?;
        db.execute("INSERT INTO nodes (id,name,tunnel_ip,public_key,status,created_at) VALUES (?1,?2,?3,?4,'active',?5)", params![input.id, input.name, input.tunnel_ip, input.public_key, now])?;
        audit(
            &db,
            actor,
            "node.create",
            &input.id,
            "node enrolled in desired state",
            now,
        )
    }

    pub fn add_server(&self, input: ServerInput, actor: &str) -> Result<()> {
        validate_id(&input.id)?;
        validate_id(&input.node_id)?;
        if input.customer_id.trim().is_empty() || input.pterodactyl_id.trim().is_empty() {
            bail!("customer_id and pterodactyl_id are required");
        }
        let now = now();
        let db = self.db()?;
        db.execute("INSERT INTO servers (id,node_id,customer_id,pterodactyl_id,state,created_at) VALUES (?1,?2,?3,?4,'draft',?5)", params![input.id,input.node_id,input.customer_id,input.pterodactyl_id,now])?;
        audit(
            &db,
            actor,
            "server.create",
            &input.id,
            "server registered in desired state",
            now,
        )
    }

    pub fn add_mapping(&self, input: MappingInput, actor: &str) -> Result<()> {
        validate_id(&input.id)?;
        validate_public_ip(&input.public_ip)?;
        if input.public_port == 0 || input.backend_port == 0 {
            bail!("ports must be in 1..=65535");
        }
        let protocol = normalize_protocol(&input.protocol)?;
        let now = now();
        let db = self.db()?;
        let exists: Option<String> = db
            .query_row(
                "SELECT id FROM servers WHERE id=?1",
                [&input.server_id],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            bail!("server {} is not registered", input.server_id);
        }
        db.execute("INSERT INTO mappings (id,server_id,public_ip,public_port,backend_port,protocol,enabled,created_at) VALUES (?1,?2,?3,?4,?5,?6,1,?7)", params![input.id,input.server_id,input.public_ip,input.public_port,input.backend_port,protocol,now])?;
        audit(
            &db,
            actor,
            "mapping.create",
            &input.id,
            "mapping reserved; network apply remains explicit",
            now,
        )
    }

    pub fn toggle_mapping(&self, id: &str, enabled: bool, actor: &str) -> Result<()> {
        validate_id(id)?;
        let now = now();
        let db = self.db()?;
        if db.execute(
            "UPDATE mappings SET enabled=?1 WHERE id=?2",
            params![enabled as i64, id],
        )? == 0
        {
            bail!("mapping {id} not found");
        }
        audit(
            &db,
            actor,
            if enabled {
                "mapping.enable"
            } else {
                "mapping.disable"
            },
            id,
            "desired state changed; network apply remains explicit",
            now,
        )
    }

    pub fn add_ban(&self, input: BanInput, actor: &str) -> Result<()> {
        validate_public_ip(&input.ip)?;
        if input.reason.trim().is_empty() {
            bail!("ban reason is required");
        }
        let now = now();
        let db = self.db()?;
        if let Some(mapping_id) = &input.mapping_id {
            validate_id(mapping_id)?;
        }
        db.execute(
            "INSERT INTO bans (ip,mapping_id,reason,expires_at,created_at) VALUES (?1,?2,?3,?4,?5)",
            params![
                input.ip,
                input.mapping_id,
                input.reason,
                input.expires_at,
                now
            ],
        )?;
        audit(
            &db,
            actor,
            "ban.create",
            &input.ip,
            "ban recorded in desired state",
            now,
        )
    }

    pub fn remove_ban(&self, id: i64, actor: &str) -> Result<()> {
        let now = now();
        let db = self.db()?;
        if db.execute("DELETE FROM bans WHERE id=?1", [id])? == 0 {
            bail!("ban {id} not found");
        }
        audit(
            &db,
            actor,
            "ban.delete",
            &id.to_string(),
            "ban removed",
            now,
        )
    }

    pub fn state(&self) -> Result<StateView> {
        let db = self.db()?;
        Ok(StateView {
            nodes: query_nodes(&db)?,
            servers: query_servers(&db)?,
            mappings: query_mappings(&db)?,
            bans: query_bans(&db)?,
        })
    }

    fn db(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.0
            .lock()
            .map_err(|_| anyhow::anyhow!("controller database lock poisoned"))
    }
}

fn query_nodes(db: &Connection) -> Result<Vec<Node>> {
    let mut s =
        db.prepare("SELECT id,name,tunnel_ip,public_key,status,created_at FROM nodes ORDER BY id")?;
    let res = s
        .query_map([], |r| {
            Ok(Node {
                id: r.get(0)?,
                name: r.get(1)?,
                tunnel_ip: r.get(2)?,
                public_key: r.get(3)?,
                status: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(res)
}

fn query_servers(db: &Connection) -> Result<Vec<Server>> {
    let mut s = db.prepare(
        "SELECT id,node_id,customer_id,pterodactyl_id,state,created_at FROM servers ORDER BY id",
    )?;
    let res = s
        .query_map([], |r| {
            Ok(Server {
                id: r.get(0)?,
                node_id: r.get(1)?,
                customer_id: r.get(2)?,
                pterodactyl_id: r.get(3)?,
                state: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(res)
}

fn query_mappings(db: &Connection) -> Result<Vec<Mapping>> {
    let mut s = db.prepare("SELECT m.id,m.server_id,s.node_id,m.public_ip,m.public_port,m.backend_port,m.protocol,m.enabled,m.created_at FROM mappings m JOIN servers s ON s.id=m.server_id ORDER BY m.public_ip,m.public_port,m.protocol")?;
    let res = s
        .query_map([], |r| {
            Ok(Mapping {
                id: r.get(0)?,
                server_id: r.get(1)?,
                node_id: r.get(2)?,
                public_ip: r.get(3)?,
                public_port: r.get(4)?,
                backend_port: r.get(5)?,
                protocol: r.get(6)?,
                enabled: r.get::<_, i64>(7)? != 0,
                created_at: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(res)
}

fn query_bans(db: &Connection) -> Result<Vec<Ban>> {
    let mut s = db.prepare(
        "SELECT id,ip,mapping_id,reason,expires_at,created_at FROM bans ORDER BY id DESC",
    )?;
    let res = s
        .query_map([], |r| {
            Ok(Ban {
                id: r.get(0)?,
                ip: r.get(1)?,
                mapping_id: r.get(2)?,
                reason: r.get(3)?,
                expires_at: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(res)
}
fn audit(
    db: &Connection,
    actor: &str,
    action: &str,
    target: &str,
    detail: &str,
    created_at: i64,
) -> Result<()> {
    db.execute(
        "INSERT INTO audit_events (actor,action,target,detail,created_at) VALUES (?1,?2,?3,?4,?5)",
        params![actor, action, target, detail, created_at],
    )?;
    Ok(())
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
fn validate_id(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
    {
        bail!("ID must be 1-64 ASCII letters, digits, '.', '_' or '-'")
    }
    Ok(())
}
fn validate_public_ip(value: &str) -> Result<()> {
    value
        .parse::<Ipv4Addr>()
        .map_err(|_| anyhow::anyhow!("expected an IPv4 address: {value}"))?;
    Ok(())
}
fn validate_tunnel_ip(value: &str) -> Result<()> {
    let ip = value
        .parse::<Ipv4Addr>()
        .map_err(|_| anyhow::anyhow!("invalid tunnel IPv4 address"))?;
    if !ip.is_private() {
        bail!("tunnel IP must be RFC1918 private IPv4")
    };
    Ok(())
}
fn normalize_protocol(value: &str) -> Result<&'static str> {
    match value.to_ascii_lowercase().as_str() {
        "tcp" => Ok("tcp"),
        "udp" => Ok("udp"),
        "both" => bail!("create one TCP and one UDP mapping; each allocation is protocol-specific"),
        _ => bail!("protocol must be tcp or udp"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_store() -> Store {
        Store::open(Path::new(":memory:")).expect("open test store")
    }

    #[test]
    fn rejects_mapping_conflicts() {
        let store = create_test_store();
        store
            .add_node(
                NodeInput {
                    id: "node-1".into(),
                    name: "Node".into(),
                    tunnel_ip: "10.100.0.2".into(),
                    public_key: "x".repeat(44),
                },
                "test",
            )
            .unwrap();
        store
            .add_server(
                ServerInput {
                    id: "srv-1".into(),
                    node_id: "node-1".into(),
                    customer_id: "c".into(),
                    pterodactyl_id: "p".into(),
                },
                "test",
            )
            .unwrap();
        let mapping = MappingInput {
            id: "map-1".into(),
            server_id: "srv-1".into(),
            public_ip: "198.51.100.10".into(),
            public_port: 25565,
            backend_port: 25565,
            protocol: "tcp".into(),
        };
        store.add_mapping(mapping.clone(), "test").unwrap();
        // Conflict on (public_ip, public_port, protocol)
        assert!(store
            .add_mapping(
                MappingInput {
                    id: "map-2".into(),
                    ..mapping
                },
                "test"
            )
            .is_err());
    }

    #[test]
    fn test_store_crud_and_audit() {
        let store = create_test_store();

        // 1. Add node
        store
            .add_node(
                NodeInput {
                    id: "node-primary".into(),
                    name: "Primary Node".into(),
                    tunnel_ip: "10.100.0.2".into(),
                    public_key: "a".repeat(44),
                },
                "admin",
            )
            .unwrap();

        // Rejects public IP as tunnel IP
        assert!(store
            .add_node(
                NodeInput {
                    id: "node-bad".into(),
                    name: "Bad Node".into(),
                    tunnel_ip: "8.8.8.8".into(),
                    public_key: "b".repeat(44),
                },
                "admin",
            )
            .is_err());

        // 2. Add server
        store
            .add_server(
                ServerInput {
                    id: "srv-mc-1".into(),
                    node_id: "node-primary".into(),
                    customer_id: "cust-123".into(),
                    pterodactyl_id: "uuid-456".into(),
                },
                "admin",
            )
            .unwrap();

        // 3. Add mappings (TCP and UDP on same public IP & port allowed)
        store
            .add_mapping(
                MappingInput {
                    id: "map-tcp".into(),
                    server_id: "srv-mc-1".into(),
                    public_ip: "198.51.100.10".into(),
                    public_port: 25565,
                    backend_port: 25565,
                    protocol: "tcp".into(),
                },
                "admin",
            )
            .unwrap();

        store
            .add_mapping(
                MappingInput {
                    id: "map-udp".into(),
                    server_id: "srv-mc-1".into(),
                    public_ip: "198.51.100.10".into(),
                    public_port: 25565,
                    backend_port: 25565,
                    protocol: "udp".into(),
                },
                "admin",
            )
            .unwrap();

        // 4. Toggle mapping
        store.toggle_mapping("map-tcp", false, "admin").unwrap();

        // 5. Add ban and remove ban
        store
            .add_ban(
                BanInput {
                    ip: "203.0.113.99".into(),
                    mapping_id: Some("map-tcp".into()),
                    reason: "DDoS attempt".into(),
                    expires_at: Some(9999999999),
                },
                "shield",
            )
            .unwrap();

        // Query state
        let state = store.state().unwrap();
        assert_eq!(state.nodes.len(), 1);
        assert_eq!(state.servers.len(), 1);
        assert_eq!(state.mappings.len(), 2);
        assert_eq!(state.bans.len(), 1);
        assert!(
            !state
                .mappings
                .iter()
                .find(|m| m.id == "map-tcp")
                .unwrap()
                .enabled
        );
        assert!(
            state
                .mappings
                .iter()
                .find(|m| m.id == "map-udp")
                .unwrap()
                .enabled
        );

        // Remove ban
        let ban_id = state.bans[0].id;
        store.remove_ban(ban_id, "admin").unwrap();
        assert_eq!(store.state().unwrap().bans.len(), 0);

        // Verify audit log has entries
        let db = store.db().unwrap();
        let audit_count: i64 = db
            .query_row("SELECT COUNT(*) FROM audit_events", [], |r| r.get(0))
            .unwrap();
        assert!(audit_count >= 6);
    }
}
