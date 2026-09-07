use anyhow::{bail, Context, Result};
use std::fs;
use std::process::Command;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayMappingRule {
    pub public_ip: String,
    pub public_port: u16,
    pub protocol: String, // "tcp" or "udp"
    pub node_tunnel_ip: String,
    pub backend_port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeMappingRule {
    pub node_tunnel_ip: String,
    pub backend_port: u16,
    pub protocol: String,
    pub container_ip: String,
    pub container_port: u16,
}

pub struct FirewallEngine;

impl FirewallEngine {
    /// Renders atomic nftables ruleset for Gateway
    pub fn render_gateway_nftables(
        rules: &[GatewayMappingRule],
        wan_iface: Option<&str>,
        wg_iface: &str,
    ) -> String {
        let mut s = String::new();
        s.push_str("#!/usr/sbin/nft -f\n");
        s.push_str("# WireNet Gateway Atomic Ruleset (Zero-Proxy Kernel Forwarding)\n\n");
        s.push_str("table inet wirenet_gateway {\n");

        // 1. Ingress DNAT Chain (PREROUTING)
        s.push_str("    chain prerouting {\n");
        s.push_str("        type nat hook prerouting priority dstnat; policy accept;\n");
        for r in rules {
            let iface_filter = match wan_iface {
                Some(wan) => format!("iifname \"{}\" ", wan),
                None => String::new(),
            };
            s.push_str(&format!(
                "        {}ip daddr {} {} dport {} dnat to {}:{}\n",
                iface_filter,
                r.public_ip,
                r.protocol.to_lowercase(),
                r.public_port,
                r.node_tunnel_ip,
                r.backend_port
            ));
        }
        s.push_str("    }\n\n");

        // 2. Forwarding Chain (FILTER)
        s.push_str("    chain forward {\n");
        s.push_str("        type filter hook forward priority filter; policy accept;\n");
        s.push_str("        ct state established,related accept\n");
        s.push_str(&format!("        iifname \"{}\" accept\n", wg_iface));

        for r in rules {
            s.push_str(&format!(
                "        oifname \"{}\" ip daddr {} {} dport {} accept\n",
                wg_iface,
                r.node_tunnel_ip,
                r.protocol.to_lowercase(),
                r.backend_port
            ));
        }
        s.push_str("    }\n");

        // NO SNAT / NO MASQUERADE on wg0 to preserve client source IP
        s.push_str("}\n");
        s
    }

    /// Renders atomic nftables ruleset for Node
    pub fn render_node_nftables(rules: &[NodeMappingRule], wg_iface: &str) -> String {
        let mut s = String::new();
        s.push_str("#!/usr/sbin/nft -f\n");
        s.push_str("# WireNet Node Atomic Ruleset (Conntrack Return Policy & Private Backend)\n\n");
        s.push_str("table inet wirenet_node {\n");

        // 1. Prerouting NAT and Connection Marking
        s.push_str("    chain prerouting {\n");
        s.push_str("        type nat hook prerouting priority dstnat; policy accept;\n");
        // Ingress wg0 connection mark (0x1) saved into conntrack
        s.push_str(&format!(
            "        iifname \"{}\" ct state new meta mark set 0x1 ct mark set 0x1\n",
            wg_iface
        ));

        // Direct container DNAT
        for r in rules {
            s.push_str(&format!(
                "        ip daddr {} {} dport {} dnat to {}:{}\n",
                r.node_tunnel_ip,
                r.protocol.to_lowercase(),
                r.backend_port,
                r.container_ip,
                r.container_port
            ));
        }
        s.push_str("    }\n\n");

        // 2. Route Mangling for Reply Symmetric Routing
        s.push_str("    chain mangle_prerouting {\n");
        s.push_str("        type filter hook prerouting priority mangle; policy accept;\n");
        s.push_str("        ct state established,related meta mark set ct mark\n");
        s.push_str("    }\n\n");

        s.push_str("    chain mangle_output {\n");
        s.push_str("        type route hook output priority mangle; policy accept;\n");
        s.push_str("        ct state established,related meta mark set ct mark\n");
        s.push_str("    }\n\n");

        // 3. Forward Filter
        s.push_str("    chain forward {\n");
        s.push_str("        type filter hook forward priority filter; policy accept;\n");
        s.push_str("        ct state established,related accept\n");
        s.push_str(&format!("        iifname \"{}\" accept\n", wg_iface));
        s.push_str(&format!("        oifname \"{}\" accept\n", wg_iface));
        s.push_str("    }\n\n");

        // 4. Backend Privacy (Drop game ports from eth0/public WAN)
        s.push_str("    chain input {\n");
        s.push_str("        type filter hook input priority filter; policy accept;\n");
        s.push_str("        iifname \"lo\" accept\n");
        s.push_str(&format!("        iifname \"{}\" accept\n", wg_iface));

        for r in rules {
            s.push_str(&format!(
                "        iifname != \"{}\" {} dport {} drop\n",
                wg_iface,
                r.protocol.to_lowercase(),
                r.backend_port
            ));
        }
        s.push_str("    }\n");

        s.push_str("}\n");
        s
    }

    /// Validates and atomically applies an nftables ruleset
    pub fn apply_nftables(ruleset: &str) -> Result<()> {
        let temp_path =
            std::env::temp_dir().join(format!("wirenet_rules_{}.nft", rand::random::<u32>()));
        fs::write(&temp_path, ruleset)
            .with_context(|| format!("writing candidate rules to {}", temp_path.display()))?;

        // 1. Syntax check before touching live state
        let check_out = Command::new("nft")
            .args(["-c", "-f", &temp_path.to_string_lossy()])
            .output()
            .context("validating nftables ruleset syntax")?;

        if !check_out.status.success() {
            let _ = fs::remove_file(&temp_path);
            bail!(
                "nftables validation failed: {}",
                String::from_utf8_lossy(&check_out.stderr)
            );
        }

        // 2. Atomic kernel apply
        let apply_out = Command::new("nft")
            .args(["-f", &temp_path.to_string_lossy()])
            .output()
            .context("applying nftables ruleset")?;

        let _ = fs::remove_file(&temp_path);

        if !apply_out.status.success() {
            bail!(
                "nftables apply failed: {}",
                String::from_utf8_lossy(&apply_out.stderr)
            );
        }

        Ok(())
    }

    /// Renders fallback iptables commands for environments without nft CLI
    pub fn render_gateway_iptables(
        rules: &[GatewayMappingRule],
        wg_iface: &str,
    ) -> Vec<Vec<String>> {
        let mut cmds = Vec::new();

        // 1. Ensure dedicated custom chains exist
        cmds.push(vec![
            "iptables".into(),
            "-t".into(),
            "nat".into(),
            "-N".into(),
            "WIRENET_GW_DNAT".into(),
        ]);
        cmds.push(vec![
            "iptables".into(),
            "-t".into(),
            "nat".into(),
            "-C".into(),
            "PREROUTING".into(),
            "-j".into(),
            "WIRENET_GW_DNAT".into(),
        ]);
        cmds.push(vec![
            "iptables".into(),
            "-t".into(),
            "nat".into(),
            "-F".into(),
            "WIRENET_GW_DNAT".into(),
        ]);

        // 2. Add DNAT rules
        for r in rules {
            cmds.push(vec![
                "iptables".into(),
                "-t".into(),
                "nat".into(),
                "-A".into(),
                "WIRENET_GW_DNAT".into(),
                "-d".into(),
                r.public_ip.clone(),
                "-p".into(),
                r.protocol.to_lowercase(),
                "--dport".into(),
                r.public_port.to_string(),
                "-j".into(),
                "DNAT".into(),
                "--to-destination".into(),
                format!("{}:{}", r.node_tunnel_ip, r.backend_port),
            ]);
        }

        // 3. Ensure wg0 forwarding is permitted
        cmds.push(vec![
            "iptables".into(),
            "-A".into(),
            "FORWARD".into(),
            "-i".into(),
            wg_iface.into(),
            "-j".into(),
            "ACCEPT".into(),
        ]);
        cmds.push(vec![
            "iptables".into(),
            "-A".into(),
            "FORWARD".into(),
            "-o".into(),
            wg_iface.into(),
            "-j".into(),
            "ACCEPT".into(),
        ]);

        cmds
    }

    /// Renders fallback iptables commands for Node
    pub fn render_node_iptables(rules: &[NodeMappingRule], wg_iface: &str) -> Vec<Vec<String>> {
        let mut cmds = Vec::new();

        // 1. Ingress marking on wg0 (0x1)
        cmds.push(vec![
            "iptables".into(),
            "-t".into(),
            "mangle".into(),
            "-A".into(),
            "PREROUTING".into(),
            "-i".into(),
            wg_iface.into(),
            "-m".into(),
            "conntrack".into(),
            "--ctstate".into(),
            "NEW".into(),
            "-j".into(),
            "CONNMARK".into(),
            "--set-mark".into(),
            "0x1".into(),
        ]);

        // 2. Restore conntrack mark on output / reply
        cmds.push(vec![
            "iptables".into(),
            "-t".into(),
            "mangle".into(),
            "-A".into(),
            "PREROUTING".into(),
            "-m".into(),
            "conntrack".into(),
            "--ctstate".into(),
            "RELATED,ESTABLISHED".into(),
            "-j".into(),
            "CONNMARK".into(),
            "--restore-mark".into(),
        ]);

        // 3. Container DNAT
        for r in rules {
            cmds.push(vec![
                "iptables".into(),
                "-t".into(),
                "nat".into(),
                "-A".into(),
                "PREROUTING".into(),
                "-d".into(),
                r.node_tunnel_ip.clone(),
                "-p".into(),
                r.protocol.to_lowercase(),
                "--dport".into(),
                r.backend_port.to_string(),
                "-j".into(),
                "DNAT".into(),
                "--to-destination".into(),
                format!("{}:{}", r.container_ip, r.container_port),
            ]);
        }

        // 4. Drop direct game ports from non-wg0 (private backend policy)
        for r in rules {
            cmds.push(vec![
                "iptables".into(),
                "-A".into(),
                "INPUT".into(),
                "!".into(),
                "-i".into(),
                wg_iface.into(),
                "-p".into(),
                r.protocol.to_lowercase(),
                "--dport".into(),
                r.backend_port.to_string(),
                "-j".into(),
                "DROP".into(),
            ]);
        }

        cmds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_gateway_nftables() {
        let rules = vec![
            GatewayMappingRule {
                public_ip: "198.51.100.10".into(),
                public_port: 25565,
                protocol: "tcp".into(),
                node_tunnel_ip: "10.200.0.2".into(),
                backend_port: 25565,
            },
            GatewayMappingRule {
                public_ip: "198.51.100.10".into(),
                public_port: 19132,
                protocol: "udp".into(),
                node_tunnel_ip: "10.200.0.2".into(),
                backend_port: 19132,
            },
        ];

        let nft = FirewallEngine::render_gateway_nftables(&rules, Some("eth0"), "wg0");
        assert!(nft.contains("table inet wirenet_gateway"));
        assert!(nft.contains(
            "iifname \"eth0\" ip daddr 198.51.100.10 tcp dport 25565 dnat to 10.200.0.2:25565"
        ));
        assert!(nft.contains(
            "iifname \"eth0\" ip daddr 198.51.100.10 udp dport 19132 dnat to 10.200.0.2:19132"
        ));
        assert!(!nft.contains("masquerade"));
        assert!(!nft.contains("snat"));
    }

    #[test]
    fn test_render_node_nftables() {
        let rules = vec![NodeMappingRule {
            node_tunnel_ip: "10.200.0.2".into(),
            backend_port: 25565,
            protocol: "tcp".into(),
            container_ip: "172.18.0.5".into(),
            container_port: 25565,
        }];

        let nft = FirewallEngine::render_node_nftables(&rules, "wg0");
        assert!(nft.contains("table inet wirenet_node"));
        assert!(nft.contains("iifname \"wg0\" ct state new meta mark set 0x1 ct mark set 0x1"));
        assert!(nft.contains("ip daddr 10.200.0.2 tcp dport 25565 dnat to 172.18.0.5:25565"));
        assert!(nft.contains("ct state established,related meta mark set ct mark"));
        assert!(nft.contains("iifname != \"wg0\" tcp dport 25565 drop"));
    }
}
