# WireNet Gap Analysis

**Baseline:** [CURRENT_ARCHITECTURE.md](CURRENT_ARCHITECTURE.md), revision `7ed9590`  
**Target:** transparent, kernel-forwarded IPv4 ingress for private Pterodactyl backends, with a durable control plane and no customer-side proxy/plugin configuration.

Severity reflects the requested priority order: security, source-IP correctness, return-path correctness, and backend privacy come first.

## Requirement comparison

| Priority | Required outcome | Current state | Gap and risk | Required action |
| --- | --- | --- | --- | --- |
| Critical | One normal packet path in the Linux kernel | Gateway service binds all game TCP ports and copies streams in Tokio, while setup also installs DNAT | Split brain; TCP proxy hides client IP and conflicts with DNAT. UDP is absent from the proxy. | **REMOVE** the normal-path listeners/forwarders; retain only a control-plane process. |
| Critical | Real source IPv4 reaches the application | No full path test exists. The userspace TCP path rewrites source IP by design. Docker rules are broad and unverified. | Claim is contradicted by executable code; IP bans cannot be trusted. | **REWRITE** data-plane configuration and prove it with namespace/container packet captures. |
| Critical | Symmetric return path via gateway | Node marks ingress and adds table 100, but rules are duplicated/mutable and no server-specific flow proof exists. | Replies may leave the provider interface or use an incorrect source; asymmetric flows fail. | **REFACTOR/REWRITE** policy-routing manager with explicit conntrack mark restore and integration tests. |
| Critical | Private backend game ports | Node setup does not install interface-specific default-deny policy; doctor adds blanket accepts; Docker-published ports remain subject to Docker defaults. | Direct node exposure and lateral access cannot be ruled out. | **REWRITE** node firewall policy; allow only management and WireNet-owned tunnel traffic. |
| Critical | Safe firewall ownership | UFW, iptables, Docker chains, wg-quick hooks, and direct commands all mutate policy. | Rule ordering, cleanup, and persistence are undefined; unrelated workloads can be exposed. | **REPLACE** WireNet rule generation with one nftables-owned table and a Docker-version adapter. Do not edit Docker-owned tables. |
| Critical | Safe changes and rollback | Setup overwrites `wg0.conf`, stops services, deletes interfaces, ignores most errors, and immediately enables services. | A failed apply can remove remote management access. | **REWRITE** as validate → stage → atomic apply → probe → commit/rollback. |
| Critical | Authenticated controller/node enrollment | TCP/9000 can bind publicly and uses a baked-in default shared token. | Any holder/guesser of the default token can register or mutate routing state. | **REPLACE** with one-time, expiring enrollment, node-generated key/certificate identity, mTLS, authorization, and audit events. |
| High | Multiple public IPs and generic TCP/UDP mappings | Gateway DNAT selects neither destination public IP nor protocol mapping identity; one static target and fixed port range are used. | Cannot allocate `IP:port:protocol`, prevent conflicts, or map two IPs using one port. | **REPLACE** with durable public-IP/port allocations and nftables concatenation maps. |
| High | Node/customer/server isolation | One flat `/24`, broad peer `AllowedIPs`, broad forward accepts, no identities, and a port-only map. | A node/customer can overlap or reach another node's traffic. | **REPLACE** with server/node/customer IDs, unique tunnel addresses, least-privilege peers, and explicit allow maps. |
| High | Pterodactyl integration | Local Docker polling only; no Panel/Wings allocation identity or lifecycle hook. | An ephemeral Docker IP can be routed to the wrong/stopped container; ownership cannot be audited. | **REPLACE** discovery with an authenticated Pterodactyl/Wings integration plus Docker reconciliation as a constrained implementation detail. |
| High | TCP and UDP support | Kernel setup attempts both; runtime proxy only supports TCP; monitoring/doctor only test TCP. | Behavior depends on which competing path wins. UDP acceptance cannot be claimed. | **REWRITE** mapping and diagnostics to be protocol-specific; test TCP and UDP end-to-end. |
| High | Durable declarative desired state | State is in generated text, mutable rules, and an in-memory map. | Restart/controller outage/agent restart lose allocation knowledge or leak rules. | **REPLACE** with a transactional database plus versioned gateway/node desired state and acknowledgements. |
| High | Secure update/install | Root builds arbitrary `main`, ignores package failures, and overwrites the live binary. | Supply-chain and availability risk. | **REPLACE** with versioned signed/reproducible release artifacts or disable self-update until that exists. |
| High | Non-destructive diagnostics | `doctor` changes sysctls/firewall/routing and can recreate wg0 before it reports results. | A read-like command changes production traffic and hides failure. | **REWRITE** diagnostics as read-only by default; a separately authorized repair plan may be applied after review. |
| Medium | API, authorization, audit | No API/database/roles/audit log. | Operations cannot be delegated, reproduced, or investigated. | **ADD** minimal authenticated API and append-only audit storage. |
| Medium | Observability | TUI approximates packet traffic and selected TCP flows; no reliable map from flow to server. | Operators cannot safely diagnose a tenant mapping or measure drops/UDP. | **REPLACE** telemetry with nftables counters, WireGuard statistics, bounded conntrack metadata, and agent/controller metrics. |
| Medium | Reboot persistence | wg-quick units are enabled, but dynamic Docker rules and sysctls are not reconciled from a durable model. | Networking may differ after reboot or container recreation. | **REFACTOR** into systemd units that load the last known-good desired state and reconcile after Docker/Wings. |
| Medium | IPv6 truthfulness | No IPv6 design or tests. | An implicit IPv6 path could bypass policy, or unsupported behavior may be advertised. | **DEFER** IPv6; explicitly ship IPv4-only with a deny/diagnostic posture until separately designed. |
| Medium | Production quality gate | One unit test exists. CI now enters the nested crate and formatting/Clippy/unit checks pass, but there is no network proof. | A green static gate cannot establish networking correctness. | **EXTEND** CI with unit/integration tests and gate release on a test topology. |
| Low | Accurate documentation | Readmes conflict with source and one another; old FRP/PROXY-protocol instructions remain. | Operators may deploy insecure, incompatible paths. | **REPLACE/REMOVE** stale material and publish the architecture and operations documents. |

## Component decisions

| Component | Current implementation | Decision | Evidence and reason |
| --- | --- | --- | --- |
| Rust as implementation language | One Rust binary with Tokio/Clap | **KEEP** | A memory-safe systems language is appropriate. The current module boundaries can be reused only after responsibilities are narrowed. |
| Linux WireGuard | Uses `wg`, `wg-quick`, kernel interface | **KEEP / REFACTOR** | WireGuard is the requested encrypted transport. Keep the kernel implementation, but replace fixed addresses, broad peers, and destructive setup. |
| Gateway userspace ingress | `gateway/server.rs` TCP listener and `copy_bidirectional` | **REMOVE** | It is a normal-path proxy, loses source IP, conflicts with DNAT, lacks UDP, and violates the non-negotiable requirement. |
| Local forwarder | `node/forwarder.rs` | **REMOVE** | Unused normal-path TCP proxy. It cannot preserve source IP and has no lifecycle or UDP support. |
| Gateway router | In-memory `port -> node` map | **REPLACE** | It has no public IP/protocol/backend port/server identity, durability, conflict guard, or authz. |
| WireGuard setup | `SetupManager` | **REWRITE** | It rewrites `wg0.conf`, assumes a topology, uses global `FORWARD ACCEPT`, and ignores operational failures. |
| Node direct-container concept | Node DNAT to container address | **REFACTOR** | Direct routing can preserve source IP, but discovery, protocol, firewall, and cleanup must be owned and verified. |
| Docker polling | `DockerWatcher` | **REPLACE** | CLI parsing, invented fallback mappings, absent removal, and no Pterodactyl identity make it unsafe. |
| Connection marking/policy routing | iptables mangle plus table 100 | **REFACTOR** | The mechanism is relevant but needs one named mark range, nftables ownership, exact ordering, idempotence, and a packet-path test. |
| In-process rate limiter | `AntiDDoSShield` | **REMOVE from data plane; optionally reuse policy ideas** | It protects only the forbidden TCP proxy, is invisible to UDP/kernel traffic, and cannot be the gateway's general firewall. |
| Shield sysctl profiles | `ShieldManager` | **REWRITE** | Host-wide unvalidated writes are not a complete DDoS defense and do not persist or audit. |
| TUI | `tui/dashboard.rs` | **REPLACE / REUSE presentation only** | Terminal rendering is fine, but its data is heuristic, fixed-port, TCP-focused, and makes unsupported health claims. |
| Doctor/status | `ops/doctor.rs`, `ops/status.rs` | **REWRITE** | Health checks must inspect, explain, and prove mappings; they must not mutate before reporting. |
| Update/uninstall | `ops/update.rs`, `ops/uninstall.rs` | **REPLACE** | Unsigned source builds and destructive removal are unsuitable for production. |
| Control protocol | Plain JSON/TCP shared token | **REPLACE** | No enrollment lifecycle, mTLS, authorization, replay protection, or durable versioning. |
| Documentation | Parent/nested docs and ignored `.ai/` | **REWRITE / REMOVE stale references** | Current instructions reference FRP, deleted scripts, HAProxy, and customer plugins contrary to the requested system. |

## Security findings

1. A default shared secret is compiled into CLI defaults and the network-facing control plane. This is a credential vulnerability, not an acceptable development convenience.
2. `doctor` and setup commands install broad accepts and global forwarding policy changes, bypassing the requested private-backend isolation.
3. Gateway peer `AllowedIPs` includes shared RFC1918 ranges, which is incompatible with least-privilege multi-peer cryptokey routing and can cause ambiguous peer selection.
4. Dynamic agent rules are added without validation, ownership tags, removal, conflict detection, or error handling.
5. Setup and update run privileged package/network/build operations with broadly ignored failures. The updater follows mutable GitHub `main` and has no signature or digest check.
6. The installer and systemd units lack a secret model and hardening boundary. The node agent must access Docker but does not constrain that powerful access.
7. The current uninstaller can remove an existing `wg0.conf` and key material without proving that WireNet owns them.
8. No audit trail exists for peer, route, firewall, or isolation actions.

## Dependencies and code hygiene

The crate declares `toml` and `thiserror` but has no source use of either. `futures` is also not directly used; `futures-util` is the used crate. Several `#[allow(dead_code)]` / `#[allow(unused_imports)]` annotations mask unused modules and imports. The behavior-neutral Clippy findings and Rust formatting drift found during inventory have been corrected; `cargo clippy --workspace --all-targets -- -D warnings` now passes.

These are cleanup items, not the primary architectural risk. Remove or replace them only as part of the corresponding component rewrite, so dependency changes do not obscure networking changes.

## Evidence limits

The inspection verified code paths and the local build/test result; it did not inspect a deployed gateway, node, Docker/Pterodactyl host, cloud firewall, or public-IP routing. Therefore, no existing "works" claim is accepted as evidence of transparent routing. The mandatory test gates are defined in [WIRENET_ARCHITECTURE.md](WIRENET_ARCHITECTURE.md) and scheduled in [WIRENET_MIGRATION.md](WIRENET_MIGRATION.md).
