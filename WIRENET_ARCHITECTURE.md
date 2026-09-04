# WireNet Recommended Architecture

**Status:** proposed target architecture; not implemented by the inspected revision.  
**Initial scope:** IPv4 transparent routing for TCP and UDP through a WireGuard gateway to private Pterodactyl nodes. IPv6 is explicitly out of scope until its parallel address, firewall, and test design is approved.

## Design principles

1. The data plane is Linux kernel routing, WireGuard, conntrack, and firewall/NAT only. A WireNet process never accepts or proxies customer game traffic.
2. The controller is off the packet path. Existing mappings keep forwarding during controller or agent outages.
3. Every allocation has an immutable server ID and versioned desired state. IP addresses and ports are attributes, never identities.
4. The firewall is default deny for new traffic. A mapping authorizes exactly its public IP, protocol, public port, node, container destination, and lifecycle.
5. A bad configuration must fail before replacing the last known-good ruleset. Apply is transactional and auditable.
6. Docker/Pterodactyl are integrations, not WireNet's networking owner. WireNet never silently changes Docker's own tables.

## Components

```text
                         management network
      administrators ---------------------------------------------+
                                                               +---v-----------------+
                                                               | WireNet Controller  |
                                                               | API + allocator     |
                                                               | audit + telemetry   |
                                                               +---+-------------+---+
                                                                   |             |
                                                      desired state|             |mTLS
                                                                   |             |
INTERNET                                                           |             |
   |                                                               |             |
   v                                                     +---------v--+      +---v-----------+
+-------------------+     encrypted WireGuard           | Gateway    |      | Node agent(s) |
| Gateway public IP |===================================>| reconciler |<---->| Pterodactyl / |
| pool + nftables   |                                    | + nftables |      | Wings adapter |
+---------+---------+                                    +---------+--+      +-------+-------+
          |                                                         |                 |
          | DNAT                                                    |                 | Docker inspect/events
          v                                                         v                 v
   public-IP:port                                  WireGuard kernel interface    container endpoint
                                                        (no userspace proxy)       (private only)
```

The initial deployment may host the controller and gateway reconciler on the same hardened gateway machine, but they remain separate processes and privileges. The controller can later move to a private management network without changing the data plane.

## Identity and durable desired state

The controller stores a small relational model. SQLite is sufficient for one controller/gateway when protected by backups and serialized writes; an HA deployment may use PostgreSQL. The data model is deliberately small.

| Resource | Required fields |
| --- | --- |
| `gateway` | `gateway_id`, management address, WireGuard address, public-IP pool, firewall backend, status |
| `node` | `node_id`, public key/certificate fingerprint, WireGuard tunnel address, management endpoint, status, config version |
| `customer` | `customer_id`, status |
| `server` | `server_id`, `customer_id`, `node_id`, Pterodactyl server UUID, lifecycle state, network policy |
| `mapping` | `mapping_id`, `server_id`, public IPv4, public port, backend port, protocol, backend container address, config version, state |
| `allocation` | unique public-IP/protocol/port key, reservation/lease timestamps, owner mapping |
| `policy` | server/customer/node scope, allow/deny/rate-limit rule, expiry, actor |
| `audit_event` | request ID, actor, action, target, old/new redacted state, result, timestamp |

The unique constraint is `(gateway_id, public_ipv4, protocol, public_port)` for active or reserved mappings. A TCP and UDP mapping may share an address and port only when the product intentionally allocates both protocols.

The controller is the only writer of desired state. Gateway and node agents acknowledge applied versions with a content digest, status, and errors. A stale agent cannot overwrite a newer mapping.

## Node enrollment and control plane

1. An administrator creates a one-time enrollment token with a short expiry, gateway/node scope, and audit record.
2. The node agent generates its WireGuard private key and an independent controller-identity keypair locally. Private keys never leave the host.
3. Over a pinned controller endpoint, the agent submits the token, node public keys, and an attestation of its initial host/network capabilities.
4. The controller consumes the token, issues a client certificate (or records a public-key identity), assigns a unique tunnel address, and creates a pending node record.
5. The agent establishes mTLS for all future control calls. It receives versioned desired state, stages it, and returns an acknowledgement.

The controller API binds only to a management interface or reverse proxy with TLS, authentication, rate limits, and role-based authorization. A gateway's public game interface never exposes the control API. All mutating calls carry a request ID and create an audit event.

## Transparent IPv4 packet path

For a mapping `198.51.100.10:25565/tcp -> node-01 (10.100.0.2):25565 -> container 172.18.0.42:25565`, the path is:

```text
client 203.0.113.50:53000
  -> gateway WAN 198.51.100.10:25565
  -> gateway nftables DNAT: 10.100.0.2:25565
       source remains 203.0.113.50:53000
  -> WireGuard peer for node-01
  -> node-01 wg0
  -> node nftables DNAT: 172.18.0.42:25565
       source still remains 203.0.113.50:53000
  -> Docker bridge -> application

application reply
  -> node conntrack reverses node DNAT to 10.100.0.2:25565
  -> restored connection mark selects policy table 100, egress wg0
  -> gateway conntrack reverses gateway DNAT to 198.51.100.10:25565
  -> client 203.0.113.50:53000
```

The application sees `203.0.113.50`, so application IP bans distinguish clients. The gateway does **not** SNAT/MASQUERADE traffic sent to `wg0`. No WireNet process listens on the public game port.

### WireGuard peer and return-route model

* The gateway gives each node a unique tunnel address, such as `10.100.0.2/32`. The gateway peer's `AllowedIPs` contains that exact node tunnel address, not shared RFC1918/container ranges.
* The node's gateway peer accepts `0.0.0.0/0` for encrypted player-source packets, while `Table = off` prevents `wg-quick` from replacing the host's main default route.
* A WireNet-reserved mark is assigned only to flows that arrived from `wg0`. The mark is saved to conntrack.
* For forwarded container replies, the node restores the conntrack mark in prerouting before route lookup. For a host-networked backend, it restores it in output as well.
* An `ip rule` matching that exact mark selects a WireNet policy table whose default route is `dev wg0`. The main table continues to carry management and ordinary node traffic.
* Reverse-path filtering is configured only where the asymmetric transparent flow requires it and is documented per interface; it is not disabled globally as a convenience.

WireGuard's cryptokey routing validates received inner addresses against peer `AllowedIPs`; peer prefixes must therefore be non-overlapping and intentionally minimal. The mechanism and its source-address check are described in the [WireGuard paper](https://www.wireguard.com/papers/wireguard.pdf).

## Firewall and NAT configuration

WireNet owns one named nftables table per role, for example `table inet wirenet_gateway` and `table inet wirenet_node`. It stages a complete generated ruleset, validates it with `nft -c -f`, then performs one nftables transaction. It does not append anonymous per-event commands.

### Gateway rules

The gateway uses a concatenation map keyed by destination public IPv4 and destination port for each protocol. Conceptually:

```text
(198.51.100.10, 25565) tcp -> (10.100.0.2, 25565)
(198.51.100.11, 25565) tcp -> (10.100.0.3, 25565)
(198.51.100.12, 19132) udp -> (10.100.0.4, 19132)
```

The map is reached only from the intended public ingress interface and only for gateway-owned/validated IP addresses. Stateful forwarding accepts established/related replies and new flows only when a mapping matches. It has counters per mapping and bounded rate-limit/temporary-block sets. It does not set a global `FORWARD ACCEPT` policy or a catch-all WAN masquerade.

Stateful NAT is appropriate here: conntrack records the NAT binding on the initial flow packet and applies it to later flow packets. See the [nftables NAT documentation](https://wiki.nftables.org/wiki-nftables/index.php/Performing_Network_Address_Translation_%28NAT%29) and [nftables conntrack metadata reference](https://wiki.nftables.org/wiki-nftables/index.php/Matching_connection_tracking_stateful_metainformation).

### Node rules

The node accepts only WireGuard ingress that matches an applied server mapping. It DNATs the node tunnel destination and backend port to the assigned container endpoint, marks the associated conntrack flow, and permits only the matching `wg0 -> container bridge` / reply path. It blocks new game connections from the public/provider interface before Docker's published-port behavior can expose a backend service. Management access is separately allowlisted and never inferred from game-port rules.

The node permits no arbitrary `wg0 -> wg0`, container-to-container, customer-to-customer, or public-interface-to-container forwarding. Emergency quarantine removes a node's gateway mappings first; local management access remains available. Server isolation removes one mapping and adds a scoped drop with an audit record.

### Docker compatibility boundary

Docker owns the rules it creates for bridge networking. WireNet must inventory the Docker firewall backend at enrollment and use a tested adapter rather than alter Docker tables. Docker documents that it creates its own filtering/NAT rules and cautions against modifying them directly; its nftables backend is currently experimental and does not have an equivalent `DOCKER-USER` chain ([Docker firewall documentation](https://docs.docker.com/engine/network/packet-filtering-firewalls/), [Docker nftables documentation](https://docs.docker.com/engine/network/firewall-nftables/)).

Initial support is restricted to a published and tested Pterodactyl/Docker backend combination. The integration test must prove that direct DNAT to the assigned container endpoint is accepted and that Docker does not re-NAT the client source. Unsupported Docker versions/backend configurations are rejected with an actionable diagnostic, not silently patched with broad accept rules.

## Pterodactyl integration

Pterodactyl remains authoritative for server lifecycle, container creation, files, resource limits, and allocations. WireNet subscribes to or queries authenticated Panel/Wings lifecycle data to associate a `server_id` with its node and allocation; it never guesses from a container display name.

When a server starts, the node agent resolves the corresponding container endpoint and reports it with a configuration version. The controller validates that the server owns the requested allocation, commits the mapping transaction, applies the gateway state, then applies node state. Container recreation triggers a staged endpoint update. When a server stops or is deleted, mappings are withdrawn and the old node rule is removed in the same desired-state reconciliation.

No customer installs a plugin, proxy, PROXY protocol adapter, or special client configuration.

## Public IP management

Public IPs are explicit gateway resources. Before an address can enter the pool, the gateway reconciler verifies it is locally assigned or provider-routed to the gateway according to the deployment model, not merely present in a database. Cloud NAT/EIP deployments require an explicit provider integration or a documented manual validation; `ip addr` alone is not proof of ownership in every cloud model.

The allocator reserves a mapping transactionally before publishing it. It validates protocol, port range, public address, server ownership, backend endpoint health, and conflicts. It releases only after the gateway/node acknowledge removal. One gateway can therefore advertise many public IPs while no public IP exists on the backend node.

## Security and operations

* Gateway and node units run with narrowly defined privileges. A dedicated reconciler helper is the only component allowed to change network state; the API/control process does not run arbitrary shell strings.
* System commands use fixed programs and structured validated arguments. IP addresses, CIDRs, ports, interfaces, protocol names, and IDs are parsed before use.
* Generated configuration includes an ownership marker and digest. WireNet only removes resources it owns.
* Secrets reside in root-readable files or a secret manager, never CLI defaults, command lines, logs, database plaintext, or Git.
* Mutations use `validate -> stage -> apply -> probe -> acknowledge`. A watchdog/rollback timer retains the last-known-good config when connectivity probes fail.
* A configuration backup and current nft/route/WireGuard snapshot are captured before each apply. Recovery uses a tested `wirenet rollback <version>` action rather than a broad uninstaller.
* Audit events cover enrollment, allocation, policy change, apply/rollback, ban, quarantine, isolate, and credential rotation.

## API and CLI boundary

The management API exposes only coherent resources: nodes, servers, mappings, policies/bans, traffic summaries, diagnostics, and isolation. It does not expose arbitrary route, shell, or firewall-rule execution.

The CLI is an authenticated API client by default. Read-only commands include `status`, `node list`, `server show`, `connections --server`, `traffic`, and `diagnostics`. Destructive commands (`node quarantine`, `server isolate`, release mapping, rollback) require explicit confirmation or a noninteractive approval flag, a request ID, authorization, and audit logging.

## Observability and diagnostics

The gateway exports WireGuard peer data, nftables counters/drops, active conntrack counts, mapping health, packets/bytes by mapping, and configuration version. The node exports the same state plus Docker/Pterodactyl endpoint reconciliation status. Flow metadata is bounded and expires with conntrack; packet payloads are never captured in the forwarding path.

`wirenet diagnostics` is read-only. `wirenet diagnose server <id>` renders the expected public IP/port, gateway mapping, peer, node endpoint, Docker backend status, policy route result, counters, and a precise PASS/WARN/FAIL condition. Explicit packet capture is a separately authorized, time-bounded diagnostic feature.

## Required implementation gates

1. Build the durable model, enrollment, and read-only inventory before changing production rules.
2. Implement gateway/node renderers with pure unit tests for validation, map generation, peer overlap, route selection, and rollback plans.
3. Build a Linux namespace integration topology with gateway, node, container, and at least three client source addresses.
4. Prove TCP and UDP source preservation, separate source-IP ban behavior, correct return path, direct-node denial, mapping conflict prevention, and controller-outage continuity.
5. Validate the supported Docker/Pterodactyl version combination and reboot/reconcile behavior.
6. Only then migrate one canary server; retain the ability to roll it back to its prior known-good mapping.
