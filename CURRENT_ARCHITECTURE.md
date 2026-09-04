# WireNet Current Architecture

**Inventory date:** 2026-09-04  
**Inspected revision:** `7ed959096eb9717991abd3aaaf8d4d16a2d27525`  
**Scope:** the nested `WireNet/` Git repository. The parent repository pins this repository as a gitlink and contains legacy, inconsistent documentation.

This document describes what the repository implements today. It is deliberately not a target design or a statement of production readiness.

## Repository and delivery layout

| Area | Current contents | Assessment |
| --- | --- | --- |
| Parent repository | A gitlink named `WireNet`, top-level Markdown, `.ai/` notes, a CI workflow | Parent Markdown contains substantial FRP-era material and is not an accurate operational reference for the nested Rust project. |
| `WireNet/daemon` | One Rust binary crate, `wirenet-daemon` | The only executable implementation. It has no workspace, library crate, database, or web frontend. |
| Installation | `WireNet/install.sh` builds from an unauthenticated GitHub checkout and copies a binary to `/usr/local/bin/wirenet` | Linux/Debian-oriented, root-only, and not release-verifiable. |
| Runtime persistence | `/etc/wireguard/wg0.conf`, key files, generated systemd units, mutable iptables/sysctl state | There is no WireNet-owned declarative state store or reconciliation journal. |
| CI | Parent `.github/workflows/ci.yml` runs formatting, Clippy, and tests | The workflow now checks out the nested repository recursively and runs the Rust commands from `WireNet/daemon`. It covers only the existing single-unit-test suite. |

The nested project was clean at the inspected revision. During this documentation pass, behavior-neutral formatting/lint corrections and CI working-directory wiring were applied. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace --locked` now pass; the test suite still contains only one unit test. Build artefacts are ignored and did not alter tracked files.

## Implemented components

| Component | Files | Current behavior |
| --- | --- | --- |
| CLI | `src/main.rs` | Clap commands for setup, run, gateway, node, TUI, doctor, status, shield, update, apply, peer, and uninstall. Most commands alter a live Linux host. |
| Gateway setup | `src/ops/setup.rs` | Creates `wg0` at `10.200.0.1/24`; writes iptables `PREROUTING` DNAT rules for one fixed node (`10.200.0.2`); enables forwarding; creates `wirenet-gateway.service`. |
| Node setup | `src/ops/setup.rs` | Creates `wg0` at `10.200.0.2/24`, configures `Table = off`, a gateway peer with `AllowedIPs = 0.0.0.0/0`, packet/connection marks, table 100, and direct DNAT rules to Docker container addresses detected at setup time. |
| Gateway runtime | `src/gateway/server.rs` | Opens a JSON control listener on TCP/9000 and creates a Tokio TCP listener for every port in a configured range (default 25565–25700). Each accepted TCP stream is copied to a node socket in userspace. It has no UDP ingress listener. |
| Control messages | `src/protocol/*`, `src/gateway/router.rs`, `src/node/agent.rs` | Length-prefixed JSON over TCP. A node registers with a caller-provided shared token, sends Docker-derived ports, and sends heartbeats. Routing data is only in memory. |
| Node agent | `src/node/agent.rs` | Every five seconds, scans Docker, inserts four iptables DNAT rules per discovered mapping, and sends port mappings to the gateway. It never removes stale rules and ignores command failures. |
| Docker discovery | `src/node/docker_watcher.rs` | Parses `docker ps` output, then calls `docker inspect`; a Unix-socket fallback only lists published ports and lacks container addresses. A final fallback invents five common mappings even when no service is listening. |
| Firewall/routing | `src/ops/setup.rs`, `src/ops/doctor.rs` | Legacy iptables commands, `wg-quick` hooks, UFW commands, Docker's own rules, and direct `ip`/`sysctl` calls coexist. No nftables ruleset is generated. |
| Protection | `src/gateway/shield.rs`, `src/ops/shield.rs` | The Tokio TCP proxy has an in-process IP token bucket. CLI shield modes change host-wide TCP/conntrack sysctls without persistence, validation, or a rollback plan. |
| Diagnostics | `src/ops/doctor.rs`, `src/ops/status.rs` | `doctor` changes state before inspecting it, may tear down/recreate `wg0`, sets permissive forwarding rules, probes a few TCP sockets, and reports completion even after failed mutations. `status` displays `wg` and service output. |
| Monitoring | `src/tui/dashboard.rs` | A terminal dashboard reads `/proc/net/dev`, `/proc/net/tcp`, and `/proc/net/nf_conntrack`. It estimates packets/sec and detects only selected TCP ports; it has no authoritative per-server flow source. |
| Updates/uninstall | `src/ops/update.rs`, `src/ops/uninstall.rs` | Update fetches and builds `main` from GitHub without version/signature verification, overwrites the binary, and restarts services. Uninstall removes `wg0.conf`, keys, and the binary without confirmation or ownership checks. |

## Current packet paths

### Intended kernel path created by setup

```text
client -> gateway public interface
       -> gateway iptables PREROUTING DNAT (any configured port -> 10.200.0.2)
       -> gateway wg0
       -> node wg0
       -> node PREROUTING DNAT (node address / wg0 traffic -> detected container IP)
       -> Docker bridge -> container
```

On the node, new packets arriving on `wg0` receive mark `0x1`; the mark is saved to conntrack. A table-100 default route over `wg0` is intended to return replies through the gateway. The gateway's original DNAT conntrack state is intended to restore the public source address on the response.

The basic ideas—DNAT without masquerading on `wg0`, connection marks, policy routing, and direct container DNAT—are recognisable components of a transparent-routing design. They are not assembled into a safe, generic, or verified implementation here.

### Actual userspace TCP path started by the gateway service

```text
client -> gateway TCP listener on 0.0.0.0:<public port>
       -> Tokio copy_bidirectional
       -> new TCP socket from gateway to 10.200.0.2:<same port>
```

This is a second data plane. Its backend sees the gateway socket's address, not the original client address. It also competes with the kernel DNAT configuration for the same public ports. It has no UDP counterpart. The historical commit message says proxy code was removed, but `gateway/server.rs` still implements this proxy.

## Current WireGuard and network assumptions

* There is exactly one gateway address, `10.200.0.1/24`, and one default node address, `10.200.0.2/24`.
* `wirenet peer add` defaults every new peer to `10.200.0.2`; it accepts a user-supplied IP but has no allocation or collision check.
* The gateway DNAT configuration always targets `10.200.0.2`, regardless of peer registration or control-plane mappings.
* The gateway peer helper grants `node-address/32, 172.16.0.0/12, 10.0.0.0/8`. Those broad, overlapping ranges are unsafe for a multi-node WireGuard cryptokey routing table.
* The node's peer uses `AllowedIPs = 0.0.0.0/0` with `Table = off`; this is intended to permit encapsulated player-source packets without replacing the host default route.
* Gateway setup adds a broad `POSTROUTING -o <default-interface> -j MASQUERADE` rule. It is not scoped to WireNet-owned egress.
* Neither setup validates public-IP ownership, interface selection, private-network overlap, peer overlap, route priority, existing firewall manager, or Docker firewall backend.
* IPv6 has no data-plane design, configuration, or tests.

## Current Docker and Pterodactyl behavior

Docker is the only integration actually present. The code reads the local Docker socket/CLI and calls containers "Pterodactyl Server" in fallback output, but it does not call the Pterodactyl Panel or Wings APIs, consume allocation events, identify a Pterodactyl server UUID, or reconcile a Pterodactyl allocation.

For a discovered published container port, the node agent inserts DNAT rules to the container IP at the public host port. These rules are duplicated on every five-second scan, do not use a WireNet-owned chain/table, and remain after a container stops or changes address. The configuration generated during setup is a one-time Docker snapshot and becomes stale as soon as Pterodactyl changes containers.

Docker owns its own firewall rules. The current code inserts a blanket `ACCEPT` at the head of `DOCKER-USER` and changes the global `FORWARD` policy to `ACCEPT`; this defeats tenant isolation and changes unrelated Docker/network traffic.

## Current control plane and data model

The control listener is bound to the configured gateway bind address, by default `0.0.0.0:9000`. Authentication is an ordinary string comparison against the default literal `wirenet_secret_token_default`. There is no TLS/mTLS, enrollment token, token expiry, certificate identity, API rate limiter, authorization model, audit log, durable node record, or server record.

The only in-memory records are a `NodeSession` and a map from port number to node ID. A port key does not include public IP, protocol, backend port, server identity, customer identity, or lifecycle state. A later sync removes prior mappings for the node but silently overwrites a port that another node already claimed.

There is no HTTP management API, database, dashboard, IP pool, port pool, route inventory, public-IP validation, ban store, quarantine operation, server isolation operation, or configuration version/acknowledgement protocol.

## Services, installation, and operations

Setup writes these units:

* `wirenet-gateway.service`: runs the gateway userspace proxy after `wg-quick@wg0`.
* `wirenet-node.service`: runs the agent with a fixed endpoint of `10.200.0.1:9000`.

Neither unit uses a dedicated account, systemd sandboxing, explicit capabilities, secret files, restart-start limits, a configuration file, or a health gate. The advertised `wirenet-watcher.service` is not created by this revision.

The installer uses `apt-get` with ignored failures, installs a Rust toolchain if missing, clones GitHub's `main`, builds it as root, and overwrites `/usr/local/bin/wirenet`. The updater performs a comparable unverified source build in `/tmp`.

## Documentation and history assessment

| Documentation area | Match to implementation |
| --- | --- |
| Nested `README.md` | Correctly identifies Rust/WireGuard/Docker themes but makes unsupported claims about real-IP delivery, security, performance, compatibility, and 0 ms discovery. |
| Nested `TROUBLESHOOTING.md` | Contradicts the zero-plugin requirement by recommending HAProxy, PROXY Protocol, and a server plugin. It also refers to deleted shell scripts. |
| Parent `README.md`, `SECURITY.md`, `TROUBLESHOOTING.md`, `CONTRIBUTING.md`, `WIREGUARD.md` | Predominantly FRP-era material and stale script references. They do not describe this repository accurately. |
| Parent `.ai/` notes | Contain useful routing concepts and ADRs, but are ignored by Git and internally contradictory: some endorse userspace ingress, proxy protocol, or masquerade on `wg0`, while others forbid them. They are not authoritative configuration. |
| Git history | Shows rapid patches to individual iptables/wg-quick commands, rather than an architecture with a declarative state model and integration tests. |

## Tests and verified limits

The test suite contains one unit test: the TCP-proxy rate limiter. `cargo test --locked` passes it. There are no tests for route selection, WireGuard peer selection, nftables/iptables rule construction, Docker lifecycle reconciliation, UDP, Pterodactyl, API authentication, allocation conflicts, return routing, real source IP delivery, reboot persistence, or rollback.

This Windows workspace is not a Linux gateway/node/container topology, so no claim about a live kernel packet path, Docker behavior, or real player IP is possible from this inspection.

## Current component classification

The evidence-based action for each component is recorded in [WIRENET_GAP_ANALYSIS.md](WIRENET_GAP_ANALYSIS.md). In short: retain the Rust implementation language and the decision to use kernel WireGuard; replace the control plane, configuration engine, firewall manager, allocation model, and diagnostics; remove the normal-path userspace proxies and stale documents.
