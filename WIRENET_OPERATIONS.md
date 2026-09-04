# WireNet Operations

## Roles

* **Controller:** owns desired state, allocation, authorization, API, audit, and telemetry aggregation. It does not forward game packets.
* **Gateway reconciler:** applies the gateway's approved versioned WireGuard, route, and nftables state.
* **Node agent:** reconciles node WireGuard/route/firewall state and reports the Pterodactyl/Docker endpoint for assigned servers.
* **Operator:** uses read-only diagnostics first, then authorized controller actions for a scoped change.

## Normal lifecycle

1. Register and attest a node using a short-lived enrollment token.
2. Add/validate public IP addresses to a gateway pool.
3. Associate a Pterodactyl server UUID with a WireNet server record.
4. Allocate a protocol-aware public mapping transactionally.
5. Wait for gateway and node version acknowledgements and endpoint health.
6. Publish the public endpoint to the customer only after the mapping is active.

## Read-only diagnostics

Read-only diagnostics show the current configuration version and concise PASS/WARN/FAIL evidence for:

* controller, gateway, and node reachability;
* WireGuard peer and recent handshake state;
* public-IP ownership and mapping uniqueness;
* nftables mapping/counter state;
* marked policy-route result on a node;
* Pterodactyl/Docker endpoint identity and freshness;
* direct-node firewall policy;
* bounded conntrack/flow summary.

A doctor command must not enable forwarding, add accepts, recreate WireGuard, or repair anything merely because it was invoked. A separate apply/repair action presents a versioned plan and requires authorization.

## Incident actions

| Action | Effect | Recovery |
| --- | --- | --- |
| Ban source | Adds a scoped, expiring or durable policy for an IP/mapping | Remove policy through audited API/CLI action. |
| Server isolate | Withdraws exactly one server's gateway/node mapping and adds a scoped deny | Reapply the prior known-good mapping version. |
| Node quarantine | Withdraws every public mapping to one node; management path remains | Restore approved node version after investigation. |
| Mapping rollback | Replaces an applied mapping/config with a retained previous version | Diagnose before retrying the failed version. |

No incident action is performed by global firewall flush, peer deletion without inventory, or an uninstaller.

## Change management

Every live change has a request ID, actor, reason, affected server/node/mapping, generated diff, validation result, applied version, health evidence, and rollback version. Scheduled maintenance uses a canary first. The migration procedure is [WIRENET_MIGRATION.md](WIRENET_MIGRATION.md).

## Backup and recovery

Back up the controller database, audit log, gateway/node desired-state versions, public-pool records, and enrollment/certificate authority material according to the hosting provider's recovery policy. Private node WireGuard keys remain on nodes and need an encrypted, access-controlled disaster-recovery process.

Recover a controller without changing existing forwarding: restore controller state, verify identities, reconcile observed gateway/node versions, then permit changes. Recover a gateway/node from its last known-good local state first; never rely on controller availability to keep a running data plane alive.
