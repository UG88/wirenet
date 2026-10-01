use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MinecraftStatus {
    pub online: bool,
    pub latency_ms: u64,
    pub version_name: String,
    pub protocol_version: i32,
    pub online_players: u32,
    pub max_players: u32,
    pub motd: String,
    pub motd_clean: String,
    pub edition: String, // "Java" or "Bedrock"
    pub player_sample: Vec<String>,
}

impl Default for MinecraftStatus {
    fn default() -> Self {
        Self {
            online: false,
            latency_ms: 0,
            version_name: "Unknown".to_string(),
            protocol_version: 0,
            online_players: 0,
            max_players: 0,
            motd: "".to_string(),
            motd_clean: "".to_string(),
            edition: "Java".to_string(),
            player_sample: Vec::new(),
        }
    }
}

/// Unified query that tries Java SLP first, then Bedrock RakNet ping if Java fails or port is 19132.
pub async fn query_minecraft_status(host: &str, port: u16) -> MinecraftStatus {
    let timeout = Duration::from_millis(1500);

    // If port is 19132 (Bedrock standard), try Bedrock first
    if port == 19132 {
        if let Ok(status) = ping_minecraft_bedrock(host, port, timeout).await {
            return status;
        }
    }

    // Try Java SLP
    if let Ok(status) = ping_minecraft_java(host, port, timeout).await {
        return status;
    }

    // Fallback to Bedrock if not tried yet
    if port != 19132 {
        if let Ok(status) = ping_minecraft_bedrock(host, port, timeout).await {
            return status;
        }
    }

    MinecraftStatus {
        online: false,
        ..Default::default()
    }
}

/// Minecraft Java Edition Server List Ping (SLP) Protocol
pub async fn ping_minecraft_java(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<MinecraftStatus> {
    tokio::time::timeout(timeout, async {
        let t_start = Instant::now();
        let mut stream = TcpStream::connect((host, port))
            .await
            .with_context(|| format!("connecting to Minecraft Java {}:{}", host, port))?;

        // 1. Send Handshake Packet (ID 0x00, NextState = 1 for Status)
        let mut handshake_body = Vec::new();
        write_varint(&mut handshake_body, 0x00); // Packet ID
        write_varint(&mut handshake_body, 765); // Protocol Version (1.20.4)
        write_string(&mut handshake_body, host); // Server address
        handshake_body.extend_from_slice(&port.to_be_bytes()); // Server port
        write_varint(&mut handshake_body, 1); // Next state: 1 (Status)

        let mut handshake_packet = Vec::new();
        write_varint(&mut handshake_packet, handshake_body.len() as i32);
        handshake_packet.extend_from_slice(&handshake_body);
        stream.write_all(&handshake_packet).await?;

        // 2. Send Status Request Packet (ID 0x00)
        let mut status_request = Vec::new();
        write_varint(&mut status_request, 1);
        status_request.push(0x00);
        stream.write_all(&status_request).await?;

        // 3. Read Status Response Packet
        let _packet_len = read_varint(&mut stream).await?;
        let packet_id = read_varint(&mut stream).await?;
        if packet_id != 0x00 {
            bail!("unexpected SLP packet id: {}", packet_id);
        }

        let json_len = read_varint(&mut stream).await?;
        if json_len <= 0 || json_len > 1_000_000 {
            bail!("invalid json response length: {}", json_len);
        }

        let mut json_bytes = vec![0u8; json_len as usize];
        stream.read_exact(&mut json_bytes).await?;
        let json_str = String::from_utf8(json_bytes)?;

        // 4. Measure ping latency
        let latency_ms = t_start.elapsed().as_millis() as u64;

        // 5. Parse JSON
        let value: serde_json::Value = serde_json::from_str(&json_str)?;

        let version_name = value["version"]["name"]
            .as_str()
            .unwrap_or("Unknown")
            .to_string();
        let protocol_version = value["version"]["protocol"].as_i64().unwrap_or(0) as i32;

        let online_players = value["players"]["online"].as_u64().unwrap_or(0) as u32;
        let max_players = value["players"]["max"].as_u64().unwrap_or(0) as u32;

        let player_sample = value["players"]["sample"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| p["name"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let motd = extract_motd_text(&value["description"]);
        let motd_clean = strip_minecraft_colors(&motd);

        Ok(MinecraftStatus {
            online: true,
            latency_ms,
            version_name,
            protocol_version,
            online_players,
            max_players,
            motd,
            motd_clean,
            edition: "Java".to_string(),
            player_sample,
        })
    })
    .await
    .context("SLP ping timed out")?
}

/// Minecraft Bedrock Edition RakNet Unconnected Ping Protocol
pub async fn ping_minecraft_bedrock(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<MinecraftStatus> {
    tokio::time::timeout(timeout, async {
        let t_start = Instant::now();
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect((host, port)).await?;

        // RakNet offline message ID magic bytes
        let magic: [u8; 16] = [
            0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe, 0xfe, 0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34,
            0x56, 0x78,
        ];

        // 1. Build RakNet Unconnected Ping packet (0x01)
        let mut ping_pkt = Vec::new();
        ping_pkt.push(0x01); // ID_UNCONNECTED_PING
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        ping_pkt.extend_from_slice(&now_ms.to_be_bytes()); // Client timestamp
        ping_pkt.extend_from_slice(&magic); // Magic
        ping_pkt.extend_from_slice(&rand::random::<u64>().to_be_bytes()); // Client GUID

        socket.send(&ping_pkt).await?;

        // 2. Read RakNet Unconnected Pong packet (0x1c)
        let mut buf = [0u8; 2048];
        let n = socket.recv(&mut buf).await?;
        let latency_ms = t_start.elapsed().as_millis() as u64;

        if n < 35 || buf[0] != 0x1c {
            bail!("invalid RakNet pong response");
        }

        // Pong structure:
        // [0]: 0x1c
        // [1..9]: timestamp (u64)
        // [9..17]: server GUID (u64)
        // [17..33]: magic (16 bytes)
        // [33..35]: string length (u16 be)
        // [35..]: server string
        let str_len = u16::from_be_bytes([buf[33], buf[34]]) as usize;
        let str_end = (35 + str_len).min(n);
        let server_str = String::from_utf8_lossy(&buf[35..str_end]);

        // Parse Bedrock server string:
        // MCPE;Server Name;Protocol Version;Version Name;Online Players;Max Players;Server Unique ID;Second MOTD;Game Mode;...
        let parts: Vec<&str> = server_str.split(';').collect();
        if parts.len() < 6 {
            bail!("invalid Bedrock server payload format");
        }

        let motd = parts.get(1).unwrap_or(&"").to_string();
        let protocol_version = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
        let version_name = parts.get(3).unwrap_or(&"Bedrock").to_string();
        let online_players = parts.get(4).and_then(|p| p.parse().ok()).unwrap_or(0);
        let max_players = parts.get(5).and_then(|p| p.parse().ok()).unwrap_or(0);
        let motd_clean = strip_minecraft_colors(&motd);

        Ok(MinecraftStatus {
            online: true,
            latency_ms,
            version_name,
            protocol_version,
            online_players,
            max_players,
            motd,
            motd_clean,
            edition: "Bedrock".to_string(),
            player_sample: Vec::new(),
        })
    })
    .await
    .context("Bedrock ping timed out")?
}

/// Minecraft Source RCON Protocol Client
pub async fn exec_rcon(
    host: &str,
    port: u16,
    password: &str,
    command: &str,
    timeout: Duration,
) -> Result<String> {
    tokio::time::timeout(timeout, async {
        let mut stream = TcpStream::connect((host, port))
            .await
            .with_context(|| format!("connecting to RCON on {}:{}", host, port))?;

        // 1. Authenticate (Type 3 = SERVERDATA_AUTH)
        let auth_id = 1424;
        let auth_packet = build_rcon_packet(auth_id, 3, password);
        stream.write_all(&auth_packet).await?;

        let (resp_id, resp_type, _body) = read_rcon_packet(&mut stream).await?;
        if resp_id == -1 || resp_type != 2 {
            bail!("RCON authentication failed (invalid password)");
        }

        // 2. Execute Command (Type 2 = SERVERDATA_EXECCOMMAND)
        let cmd_id = 1425;
        let cmd_packet = build_rcon_packet(cmd_id, 2, command);
        stream.write_all(&cmd_packet).await?;

        let (exec_resp_id, _resp_type, output) = read_rcon_packet(&mut stream).await?;
        if exec_resp_id != cmd_id {
            bail!("mismatched RCON request ID in execution response");
        }

        Ok(output)
    })
    .await
    .context("RCON operation timed out")?
}

fn build_rcon_packet(request_id: i32, packet_type: i32, body: &str) -> Vec<u8> {
    let body_bytes = body.as_bytes();
    // packet length = 4 (id) + 4 (type) + body_bytes.len() + 2 (null terminators)
    let length = (4 + 4 + body_bytes.len() + 2) as i32;

    let mut packet = Vec::with_capacity((length + 4) as usize);
    packet.extend_from_slice(&length.to_le_bytes());
    packet.extend_from_slice(&request_id.to_le_bytes());
    packet.extend_from_slice(&packet_type.to_le_bytes());
    packet.extend_from_slice(body_bytes);
    packet.push(0x00);
    packet.push(0x00);
    packet
}

async fn read_rcon_packet<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<(i32, i32, String)> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let length = i32::from_le_bytes(len_buf);
    if !(10..=65536).contains(&length) {
        bail!("invalid RCON packet length: {}", length);
    }

    let mut id_buf = [0u8; 4];
    reader.read_exact(&mut id_buf).await?;
    let request_id = i32::from_le_bytes(id_buf);

    let mut type_buf = [0u8; 4];
    reader.read_exact(&mut type_buf).await?;
    let packet_type = i32::from_le_bytes(type_buf);

    let body_len = (length - 10) as usize;
    let mut body_bytes = vec![0u8; body_len];
    if body_len > 0 {
        reader.read_exact(&mut body_bytes).await?;
    }

    // Read 2-byte null terminator
    let mut pad = [0u8; 2];
    reader.read_exact(&mut pad).await?;

    let body_str = String::from_utf8_lossy(&body_bytes).to_string();
    Ok((request_id, packet_type, body_str))
}

pub fn write_varint(buf: &mut Vec<u8>, value: i32) {
    let mut uval = value as u32;
    loop {
        let mut byte = (uval & 0x7F) as u8;
        uval >>= 7;
        if uval != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if uval == 0 {
            break;
        }
    }
}

pub async fn read_varint<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<i32> {
    let mut num_read = 0;
    let mut result = 0i32;
    loop {
        let byte = reader.read_u8().await?;
        let value = (byte & 0x7F) as i32;
        result |= value << (7 * num_read);
        num_read += 1;
        if num_read > 5 {
            bail!("VarInt is too big");
        }
        if (byte & 0x80) == 0 {
            break;
        }
    }
    Ok(result)
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    write_varint(buf, bytes.len() as i32);
    buf.extend_from_slice(bytes);
}

fn extract_motd_text(desc: &serde_json::Value) -> String {
    if let Some(s) = desc.as_str() {
        return s.to_string();
    }
    if let Some(obj) = desc.as_object() {
        let mut text = obj
            .get("text")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(extra) = obj.get("extra").and_then(|e| e.as_array()) {
            for item in extra {
                if let Some(t) = item.get("text").and_then(|s| s.as_str()) {
                    text.push_str(t);
                } else if let Some(s) = item.as_str() {
                    text.push_str(s);
                }
            }
        }
        return text;
    }
    "".to_string()
}

pub fn strip_minecraft_colors(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '§' || c == '&' {
            if let Some(&next) = chars.peek() {
                if next.is_ascii_hexdigit() || "klmnoKLMNOrR".contains(next) {
                    chars.next();
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_serialization() {
        let mut buf = Vec::new();
        write_varint(&mut buf, 0);
        assert_eq!(buf, vec![0x00]);

        buf.clear();
        write_varint(&mut buf, 25565);
        assert_eq!(buf, vec![0xdd, 0xc7, 0x01]);

        buf.clear();
        write_varint(&mut buf, -1);
        assert_eq!(buf, vec![0xff, 0xff, 0xff, 0xff, 0x0f]);
    }

    #[test]
    fn test_strip_minecraft_colors() {
        let input = "§aWelcome to §bWireNet §cServer! §lJOIN NOW§r";
        assert_eq!(
            strip_minecraft_colors(input),
            "Welcome to WireNet Server! JOIN NOW"
        );

        let input_amp = "&aSurvival &6Network &r[1.20]";
        assert_eq!(strip_minecraft_colors(input_amp), "Survival Network [1.20]");
    }

    #[test]
    fn test_rcon_packet_builder() {
        let pkt = build_rcon_packet(1, 3, "secret");
        // length = 4 (id) + 4 (type) + 6 ("secret") + 2 (nulls) = 16
        assert_eq!(i32::from_le_bytes(pkt[0..4].try_into().unwrap()), 16);
        assert_eq!(i32::from_le_bytes(pkt[4..8].try_into().unwrap()), 1);
        assert_eq!(i32::from_le_bytes(pkt[8..12].try_into().unwrap()), 3);
        assert_eq!(&pkt[12..18], b"secret");
        assert_eq!(pkt[18], 0);
        assert_eq!(pkt[19], 0);
    }

    #[test]
    fn test_extract_motd_json() {
        let v: serde_json::Value = serde_json::json!({
            "text": "Hypixel ",
            "extra": [
                { "text": "Network " },
                { "text": "[1.20.4]" }
            ]
        });
        assert_eq!(extract_motd_text(&v), "Hypixel Network [1.20.4]");
    }
}
