use anyhow::{Context, Result};
use std::process::Command;

pub struct UpdateManager;

impl UpdateManager {
    pub fn check_update() -> Result<()> {
        println!("==========================================================");
        println!(" 🔍 Checking for WireNet Updates...");
        println!("==========================================================");
        println!(" Installed Version: v{}", env!("CARGO_PKG_VERSION"));
        println!(" Upstream Branch   : main (https://github.com/UG88/wirenet)");

        let out = Command::new("git")
            .args(["ls-remote", "https://github.com/UG88/wirenet.git", "HEAD"])
            .output();
        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout);
            if let Some(hash) = s.split_whitespace().next() {
                println!(" Latest Remote Commit: {}", &hash[..8.min(hash.len())]);
            }
        }
        println!("==========================================================");
        println!(" Run 'wirenet update' to synchronize to the latest build!");
        println!("==========================================================");
        Ok(())
    }

    pub fn self_update() -> Result<()> {
        println!("==========================================================");
        println!(" 🔄 WireNet 1-Click Self-Updater");
        println!("==========================================================");

        println!("[1/4] Pulling latest repository files from GitHub...");
        let tmp_dir = "/tmp/wirenet_update";
        let _ = std::fs::remove_dir_all(tmp_dir);

        let clone = Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                "https://github.com/UG88/wirenet.git",
                tmp_dir,
            ])
            .output();
        if let Ok(c) = clone {
            if !c.status.success() {
                // Fallback tarball
                let _ = Command::new("curl")
                    .args([
                        "-fsSL",
                        "https://github.com/UG88/wirenet/archive/refs/heads/main.tar.gz",
                        "-o",
                        "/tmp/wirenet.tar.gz",
                    ])
                    .output();
                let _ = Command::new("tar")
                    .args(["-xzf", "/tmp/wirenet.tar.gz", "-C", "/tmp/"])
                    .output();
            }
        }

        println!("[2/4] Compiling optimized release binary (takes ~45-60s on 1-CPU servers)...");
        let cargo_bin = dirs_home_cargo_bin();
        let status = Command::new(&cargo_bin)
            .args(["build", "--release"])
            .current_dir(format!("{}/daemon", tmp_dir))
            .status()
            .context("Failed to run cargo build --release")?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "Cargo build failed with exit code {:?}",
                status.code()
            ));
        }

        println!("[3/4] Installing updated binary to /usr/local/bin/wirenet...");
        let target_bin = format!("{}/daemon/target/release/wirenet-daemon", tmp_dir);
        let _ = std::fs::copy(&target_bin, "/usr/local/bin/wirenet");
        let _ = Command::new("chmod")
            .args(["+x", "/usr/local/bin/wirenet"])
            .output();

        println!("[4/4] Restarting active WireNet services...");
        let _ = Command::new("systemctl")
            .args(["restart", "wirenet-gateway.service"])
            .output();
        let _ = Command::new("systemctl")
            .args(["restart", "wirenet-node.service"])
            .output();

        let _ = std::fs::remove_dir_all(tmp_dir);

        println!("==========================================================");
        println!(" [✓] WireNet Successfully Updated to Latest Version!");
        println!("==========================================================");

        Ok(())
    }
}

fn dirs_home_cargo_bin() -> String {
    if let Ok(h) = std::env::var("HOME") {
        let p = format!("{}/.cargo/bin/cargo", h);
        if std::path::Path::new(&p).exists() {
            return p;
        }
    }
    "cargo".to_string()
}
