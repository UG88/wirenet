# WireNet Migration Plan

**Purpose:** move from the inspected mutable iptables/userspace-proxy implementation to the architecture in [WIRENET_ARCHITECTURE.md](WIRENET_ARCHITECTURE.md) without treating a live gateway as a test environment.

The current `setup`, `apply`, `doctor`, `update`, and `uninstall` commands are not migration tools. Do not run them on a production gateway/node as part of this plan. In particular, do not flush tables, delete `wg0`, overwrite `/etc/wireguard/wg0.conf`, or change a global forwarding policy to discover the current state.

## Preconditions and stop conditions

Before any production change, collect a read-only per-host snapshot:

```text
nft list ruleset
iptables-save / ip6tables-save (if present)
ip -details address; ip route show table all; ip rule show
wg show all dump
sysctl values used by forwarding/rp_filter
docker info; docker network inspect; docker ps/inspect
systemctl status and unit files for WireNet, wg-quick, Docker, Wings, UFW/firewalld
cloud security-group/load-balancer/public-IP routing configuration
Pterodactyl server/allocation/node identifiers
```

Store the snapshot securely with a change request and redaction for keys/tokens. Identify the real management path to every host and obtain out-of-band console access. Never assume an SSH route survives an experimental policy-routing change.

Stop a deployment immediately if any of these is true:

* the public IP is not demonstrably owned/routed to the proposed gateway;
* current firewall/routing ownership is unknown;
* gateway/node/container test captures do not match the planned flow;
* WireGuard peer prefixes overlap or container/network CIDRs conflict;
* the Docker firewall backend is unsupported by the tested adapter;
* a rollback path or console access is unavailable.

## Phase 0 — freeze and inventory

1. Freeze new customer network allocations on the candidate gateway; do not stop existing game services.
2. Run the read-only inventory and record every public `IP:protocol:port -> node -> backend` mapping, including mappings not created by WireNet.
3. Record owners and purposes of existing UFW, iptables, nftables, Docker, cloud-security-group, and systemd configuration. Decide the explicit ownership boundary rather than mixing tools.
4. Identify all Pterodactyl nodes, servers, allocations, Docker network modes, and public management endpoints.
5. Correct the source-of-truth documentation before directing operators to any command. Retire stale FRP/HAProxy/PROXY-protocol instructions.

**Exit gate:** a reviewed inventory has no unknown public game port, no overlapping peer prefix, and a tested out-of-band recovery channel for gateway and node.

## Phase 1 — build and test the replacement outside production

1. Implement the controller data model, allocator, mTLS enrollment, audit log, and versioned desired-state protocol. The controller is not in the data path.
2. Implement gateway and node renderers that produce an owned, complete ruleset in a staging location. They must reject invalid addresses, ports, interfaces, peer overlaps, Docker backends, and allocation conflicts.
3. Implement an agent apply protocol: preflight, stage, validate, atomic apply, local probe, controller acknowledgement, retained previous version, and timed rollback if the confirmation probe fails.
4. Create a Linux network-namespace test topology comprising a client namespace, gateway namespace, WireGuard link, node namespace, Docker-compatible bridge/container endpoint, and three distinct client source addresses.
5. Test TCP and UDP mappings, public-IP selection, different backend ports, three source IPs, a per-source block, node direct-access denial, server isolation, node quarantine, controller outage, agent restart, WireGuard restart, Docker/container restart, node restart, and gateway restart.

**Exit gate:** automated tests and packet captures prove the complete path; CI runs formatting, Clippy, unit tests, and Linux integration tests from the correct working directory.

## Phase 2 — passive production readiness

1. Deploy the controller on a private management endpoint and enroll no production node until its certificate/audit/event path is verified.
2. Deploy the new gateway and node agents in **observe-only** mode. They may inventory WireGuard/Docker/Pterodactyl and render a candidate configuration but must not write network state.
3. Import current mappings as disabled/draft records. Resolve conflicts and assign actual `server_id`, `customer_id`, Pterodactyl UUID, node ID, public IP, public port, protocol, backend port, and endpoint.
4. Compare rendered rules to the read-only live snapshot. Any rule outside WireNet's owned table/map is an error, not an invitation to overwrite unrelated policy.
5. Validate public IP ownership and route reachability from an independent test client. Validate Docker backend compatibility on each node.

**Exit gate:** every candidate mapping is reconciled to a unique Pterodactyl server and an agent has reported a healthy endpoint/config digest without touching packet forwarding.

## Phase 3 — canary migration

1. Select one low-risk server with an announced maintenance window and a known test client. Reserve a spare public IP/port when feasible, so the first canary does not replace a busy production mapping.
2. Apply the gateway mapping and node endpoint as one versioned transaction. Maintain the existing production mapping until the new dedicated canary address has passed tests.
3. Capture only headers/metadata at gateway WAN, gateway `wg0`, node `wg0`, node bridge, and container interface. Verify:
   * client source address is unchanged at every forward stage;
   * destination changes only at the intended DNAT stages;
   * reply leaves node `wg0` and exits the gateway public interface;
   * the application sees each test client's distinct source IP;
   * direct provider-interface access to the backend game port is dropped;
   * TCP and UDP both work if both are allocated.
4. Test a scoped gateway/server ban for one test source. Verify the other two sources still work.
5. Leave the canary under normal traffic long enough to validate counters, conntrack expiry, agent reconciliation, and observability.

**Rollback:** remove only the canary mapping using its prior config version; confirm old production mappings and management access remain unchanged. Do not run a global uninstall, flush a NAT chain, or restart Docker/Wings as a rollback shortcut.

**Exit gate:** source preservation, return routing, isolation, monitoring, and rollback are all proven for the actual gateway/node/Docker combination.

## Phase 4 — progressive cutover

1. Migrate mappings in small, reversible batches, separated by node and customer. Announce a narrow maintenance window for any mapping that must reuse an existing public `IP:port`.
2. For a reused address, stage and validate the new mapping first, capture current state, apply atomically, probe from independent clients, and retain the previous mapping version until the observation window expires.
3. Monitor per-mapping counters, dropped packets, WireGuard handshakes, policy-route probes, application-visible source IPs, and direct-node firewall drops.
4. Quarantine a node only through the controller action. It must withdraw that node's gateway mappings while leaving management access available for recovery.
5. Stop batch rollout on any unexpected source translation, asymmetric response, collision, unowned firewall mutation, controller/agent version mismatch, or unexplained drop.

**Exit gate:** all migrated mappings have a healthy acknowledged version, expected traffic, audited allocation records, and a successful direct-node denial test.

## Phase 5 — retire legacy mechanisms

Only after all mappings are migrated and the observation period has elapsed:

1. Disable and remove WireNet's legacy userspace TCP gateway service and unused local forwarder code. Confirm no public game port is bound by a proxy.
2. Remove only legacy WireNet-owned iptables/UFW/wg-quick hooks after comparing their exact ownership markers with the recorded snapshot. Preserve unrelated Docker, host firewall, cloud, and management rules.
3. Replace the destructive self-updater/uninstaller with versioned release/install/rollback procedures.
4. Remove FRP, HAProxy, rinetd, PROXY-protocol, deleted-script, and fixed-address instructions from current documentation. Do not uninstall an administrator-owned third-party service merely because its name appears in old setup code.
5. Archive redacted snapshots, migration audit events, test evidence, and the documented rollback procedure.

## Reboot and failure recovery validation

Before declaring the migration complete, test on a noncritical/canary deployment:

| Failure | Required result |
| --- | --- |
| Controller unavailable | Existing kernel mappings and established/new permitted flows continue; mutations queue/fail clearly. |
| Agent unavailable | Last acknowledged configuration remains; controller reports stale health. |
| Gateway restart | WireGuard, owned nftables rules, mappings, and counters recover from last known-good state. |
| Node restart | Node restores the marked return route and applies current container endpoint only after Docker/Wings is ready. |
| Docker/container restart | Agent withdraws stale endpoint then safely applies the new endpoint; no duplicate/stale rule accumulation. |
| Invalid candidate config | Validation fails before live replacement or rollback restores the previous ruleset and management connectivity. |
| Public IP withdrawn | Mapping is disabled/audited and clients receive a clear health failure; no alternate unintended address is advertised. |

## Completion criteria

The migration is complete only when the acceptance tests in the requested specification have recorded evidence for the supported deployment matrix. A successful WireGuard handshake, a green service unit, or a single local TCP probe is not sufficient evidence.
