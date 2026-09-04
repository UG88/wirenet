use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldTelemetry {
    pub total_connections: u64,
    pub active_connections: u64,
    pub dropped_packets: u64,
    pub dropped_syn_floods: u64,
    pub active_tracked_ips: usize,
    pub shield_mode: String,
}

struct IpRateState {
    tokens: f64,
    last_update: Instant,
    active_count: u32,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct AntiDDoSShield {
    rate_limit: u32,
    burst_limit: u32,
    mode_name: Arc<tokio::sync::RwLock<String>>,
    ip_table: Arc<DashMap<IpAddr, IpRateState>>,
    total_conns: Arc<AtomicU64>,
    active_conns: Arc<AtomicU64>,
    dropped_attacks: Arc<AtomicU64>,
    dropped_syns: Arc<AtomicU64>,
}

impl AntiDDoSShield {
    pub fn new(mode: &str, syn_limit: u32) -> Self {
        let (rate, burst) = match mode {
            "strict" => (10, 20),
            "disabled" | "off" => (1_000_000, 1_000_000),
            _ => (syn_limit.max(5), (syn_limit * 2).max(10)),
        };

        Self {
            rate_limit: rate,
            burst_limit: burst,
            mode_name: Arc::new(tokio::sync::RwLock::new(mode.to_string())),
            ip_table: Arc::new(DashMap::new()),
            total_conns: Arc::new(AtomicU64::new(0)),
            active_conns: Arc::new(AtomicU64::new(0)),
            dropped_attacks: Arc::new(AtomicU64::new(0)),
            dropped_syns: Arc::new(AtomicU64::new(0)),
        }
    }

    #[allow(dead_code)]
    pub fn set_mode(&self, mode: &str) {
        let mut w = self.mode_name.blocking_write();
        *w = mode.to_string();
    }

    pub fn check_connection(&self, ip: IpAddr) -> bool {
        self.total_conns.fetch_add(1, Ordering::Relaxed);
        let now = Instant::now();

        let mut entry = self.ip_table.entry(ip).or_insert(IpRateState {
            tokens: self.burst_limit as f64,
            last_update: now,
            active_count: 0,
        });

        let elapsed = now.duration_since(entry.last_update).as_secs_f64();
        entry.tokens =
            (entry.tokens + elapsed * (self.rate_limit as f64)).min(self.burst_limit as f64);
        entry.last_update = now;

        if entry.tokens >= 1.0 {
            entry.tokens -= 1.0;
            entry.active_count += 1;
            self.active_conns.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            self.dropped_attacks.fetch_add(1, Ordering::Relaxed);
            self.dropped_syns.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    pub fn connection_closed(&self, ip: IpAddr) {
        if let Some(mut entry) = self.ip_table.get_mut(&ip) {
            if entry.active_count > 0 {
                entry.active_count -= 1;
            }
        }
        if self.active_conns.load(Ordering::Relaxed) > 0 {
            self.active_conns.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub async fn cleanup_stale_ips(&self) {
        let now = Instant::now();
        self.ip_table.retain(|_, state| {
            now.duration_since(state.last_update) < Duration::from_secs(60)
                || state.active_count > 0
        });
    }

    #[allow(dead_code)]
    pub async fn get_telemetry(&self) -> ShieldTelemetry {
        ShieldTelemetry {
            total_connections: self.total_conns.load(Ordering::Relaxed),
            active_connections: self.active_conns.load(Ordering::Relaxed),
            dropped_packets: self.dropped_attacks.load(Ordering::Relaxed),
            dropped_syn_floods: self.dropped_syns.load(Ordering::Relaxed),
            active_tracked_ips: self.ip_table.len(),
            shield_mode: self.mode_name.read().await.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shield_rate_limiting() {
        let shield = AntiDDoSShield::new("strict", 5);
        let test_ip = "1.2.3.4".parse().unwrap();

        // Under strict mode (rate 10, burst 20), first 20 should pass
        for _ in 0..20 {
            assert!(shield.check_connection(test_ip));
        }

        // 21st rapid connection should be blocked
        assert!(!shield.check_connection(test_ip));
    }
}
