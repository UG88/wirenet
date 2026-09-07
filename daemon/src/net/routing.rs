use anyhow::{bail, Context, Result};
use std::process::Command;

pub struct PolicyRoutingManager;

impl PolicyRoutingManager {
    /// Enables IPv4 kernel forwarding required for packet transit
    pub fn enable_ip_forwarding() -> Result<()> {
        let _ = Command::new("sysctl")
            .args(["-w", "net.ipv4.ip_forward=1"])
            .output();
        let _ = Command::new("sysctl")
            .args(["-w", "net.ipv4.conf.all.forwarding=1"])
            .output();
        Ok(())
    }

    /// Sets reverse path filtering mode (e.g. 0=disabled, 2=loose)
    pub fn set_rp_filter(iface: &str, mode: u8) -> Result<()> {
        let key = format!("net.ipv4.conf.{}.rp_filter", iface);
        let val = mode.to_string();
        let out = Command::new("sysctl")
            .args(["-w", &format!("{}={}", key, val)])
            .output()
            .with_context(|| format!("setting sysctl {}={}", key, val))?;

        if !out.status.success() {
            bail!(
                "failed setting rp_filter: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }

    /// Checks if a policy routing rule exists for the given fwmark and table
    pub fn rule_exists(table_id: u32, mark: u32) -> bool {
        let out = Command::new("ip").args(["rule", "show"]).output();
        if let Ok(res) = out {
            let s = String::from_utf8_lossy(&res.stdout);
            let mark_hex = format!("{:#x}", mark);
            let mark_dec = format!("{}", mark);
            let table_str = format!("{}", table_id);

            for line in s.lines() {
                if (line.contains(&format!("fwmark {}", mark_hex))
                    || line.contains(&format!("fwmark {}", mark_dec)))
                    && (line.contains(&format!("lookup {}", table_str))
                        || line.contains(&format!("table {}", table_str)))
                {
                    return true;
                }
            }
        }
        false
    }

    /// Installs table 100 default route and fwmark 0x1 policy rule for reply routing
    pub fn setup_node_policy_routing(
        table_id: u32,
        mark: u32,
        pref: u32,
        dev: &str,
        gateway_ip: Option<&str>,
    ) -> Result<()> {
        Self::enable_ip_forwarding()?;

        // Ensure loose or disabled reverse path filtering on wireguard interface
        let _ = Self::set_rp_filter("all", 2);
        let _ = Self::set_rp_filter("default", 2);
        let _ = Self::set_rp_filter(dev, 0);

        // 1. Add route in designated routing table
        let mut route_args = vec!["route", "replace", "default"];
        if let Some(gw) = gateway_ip {
            route_args.push("via");
            route_args.push(gw);
        }
        route_args.push("dev");
        route_args.push(dev);

        let table_str = table_id.to_string();
        route_args.push("table");
        route_args.push(&table_str);

        let route_out = Command::new("ip")
            .args(&route_args)
            .output()
            .with_context(|| format!("adding default route via {} to table {}", dev, table_id))?;

        if !route_out.status.success() {
            bail!(
                "failed adding default route to table {}: {}",
                table_id,
                String::from_utf8_lossy(&route_out.stderr)
            );
        }

        // 2. Add ip rule idempotently
        if !Self::rule_exists(table_id, mark) {
            let mark_hex = format!("{:#x}", mark);
            let pref_str = pref.to_string();
            let rule_out = Command::new("ip")
                .args([
                    "rule", "add", "fwmark", &mark_hex, "table", &table_str, "pref", &pref_str,
                ])
                .output()
                .with_context(|| {
                    format!("adding ip rule for fwmark {} table {}", mark_hex, table_id)
                })?;

            if !rule_out.status.success() {
                bail!(
                    "failed adding ip rule: {}",
                    String::from_utf8_lossy(&rule_out.stderr)
                );
            }
        }

        Ok(())
    }

    /// Removes policy routing table and matching fwmark rule
    pub fn teardown_node_policy_routing(table_id: u32, mark: u32) -> Result<()> {
        let table_str = table_id.to_string();
        let _ = Command::new("ip")
            .args(["route", "flush", "table", &table_str])
            .output();

        while Self::rule_exists(table_id, mark) {
            let mark_hex = format!("{:#x}", mark);
            let res = Command::new("ip")
                .args(["rule", "del", "fwmark", &mark_hex, "table", &table_str])
                .output();
            if res.is_err() || !res.unwrap().status.success() {
                break;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_rule_formatting() {
        assert_eq!(format!("{:#x}", 0x1), "0x1");
        assert_eq!(format!("{}", 100), "100");
    }
}
