# WireNet Security Model

This is the security baseline for the target architecture in [WIRENET_ARCHITECTURE.md](WIRENET_ARCHITECTURE.md). It supersedes the repository's legacy FRP and shared-token guidance.

## Trust boundaries

| Boundary | Trust level | Required control |
| --- | --- | --- |
| Internet to gateway game addresses | Untrusted | Gateway default deny, explicit active mapping, protocol-aware rate/connection limits, counters, no management API. |
| Gateway to node over WireGuard | Authenticated transport | Unique peer keys and non-overlapping `AllowedIPs`; WireGuard private keys remain local. |
| Controller to agent | Privileged management | mTLS, short-lived enrollment token, versioned desired state, least-privilege authorization, audit record. |
| Node to Docker/Pterodactyl | Privileged local integration | A constrained agent identity, authenticated Pterodactyl/Wings integration, documented Docker firewall adapter, endpoint ownership validation. |
| Administrator to API/CLI | Privileged human/service | TLS, RBAC, MFA/SSO where available, request IDs, confirmation for destructive actions, audit events. |
| Customer workload to another workload | Untrusted lateral traffic | No flat tenant network; mapping-specific forwarding only and explicit inter-server policy when approved. |

## Required controls

* **Node enrollment:** one-time expiring token, node-local key generation, token consumption, mTLS thereafter, credential rotation/revocation.
* **Authorization:** separate read, network-admin, security-admin, and incident-response roles. An API token is never a substitute for a node identity.
* **Secrets:** private keys and controller credentials are root-readable files or secret-manager values. They are redacted from logs and never supplied through a default CLI argument.
* **Public exposure:** only WireGuard UDP, explicitly allocated game mappings, and an intentionally protected management endpoint are permitted. The controller API is not public by default.
* **Node privacy:** new game traffic on provider/public interfaces is dropped. Tunnel traffic is accepted only if it matches a current owned mapping; management is separately allowlisted.
* **Firewall ownership:** WireNet changes only its own named nftables table and validated routes/rules. It does not flush shared tables, set a global `FORWARD ACCEPT`, or modify Docker-owned rules.
* **Command safety:** all values are parsed before use; network commands are structured, time-bounded, logged with redaction, and checked for success. No user input becomes a shell string.
* **Audit:** enrollment, allocation, apply, rollback, ban, isolate, quarantine, credential change, and failed authorization are appended with actor, request ID, target, result, and redacted before/after state.
* **Emergency isolation:** mapping removal is atomic and scoped. Node quarantine preserves an explicit management path; it never depends on deleting the tunnel blindly.
* **Updates:** accept a pinned/signed release artifact or a verified package repository. Do not build mutable remote source as root in production.

## Configuration safety

Every mutation follows this sequence:

```text
validate inputs and ownership
  -> render complete candidate state
  -> syntax/semantic validate candidate
  -> snapshot last-known-good state
  -> atomically apply owned state
  -> probe management + mapping health
  -> acknowledge and audit
  -> retain or rollback
```

Rollback is an explicit operation against a recorded configuration version. It must restore only WireNet-owned resources. A broad uninstaller, a NAT table flush, or a `wg0` deletion is never an automatic recovery action.

## Threats excluded from product claims

WireNet can reduce backend exposure and enforce network policy; it does not make an upstream DDoS disappear. Volumetric attacks that saturate the gateway uplink require provider/upstream mitigation. It also does not secure a compromised host, a malicious administrator, an insecure customer application, or an exposed Docker socket. These risks require independent host hardening, access control, updates, backups, monitoring, and incident response.

## Security acceptance checks

1. A node cannot register with an expired, consumed, or invalid enrollment token.
2. A node cannot impersonate another node after enrollment.
3. A client cannot reach a backend game port through the node provider address.
4. A mapping for one server cannot deliver to another server/customer endpoint.
5. An unallocated public IP/port/protocol tuple is dropped and counted.
6. A failed configuration cannot remove the administrator's management path.
7. A quarantine/isolation action is visible in audit logs and is reversible through an authorized operation.
8. Controller outage does not remove already-applied kernel forwarding state.
