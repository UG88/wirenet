# WireNet Test Plan

## Test environments

Unit tests validate input parsing, allocation uniqueness, desired-state rendering, peer-prefix overlap rejection, route/firewall plan generation, audit events, and rollback selection.

Linux integration tests build this topology without touching production:

```text
three client namespaces -> gateway namespace -> WireGuard -> node namespace -> container endpoint
```

The integration environment uses distinct client source addresses, a public-IP pool with at least two addresses, a TCP echo/Minecraft-compatible endpoint, a UDP echo/Bedrock-compatible endpoint, and a Docker/Pterodactyl-compatible supported backend.

## Mandatory test matrix

| Scenario | Evidence required |
| --- | --- |
| TCP mapping | Client receives response through public IP; application sees exact client source address. |
| UDP mapping | Datagram request/response succeeds; application sees exact client source address. |
| Multiple clients | Three sources reach one mapping and retain separate addresses. |
| Real-IP ban | Block one source; the other two remain connected/allowed. |
| Multiple public IPs | Same port on distinct public addresses reaches distinct nodes/servers. |
| Port translation | A public port reaches a different backend port. |
| Conflict prevention | Duplicate active `(IP, protocol, port)` allocation is rejected transactionally. |
| Return route | Packet captures show node reply on wg0 and gateway reply on public interface. |
| Direct node denial | Node public/provider interface drops new game traffic. |
| Docker endpoint update | Container restart/readdress removes old endpoint and activates the validated new one. |
| Pterodactyl identity | Mapping cannot attach to a container not owned by the expected server UUID. |
| Controller outage | Existing mapping continues forwarding; mutations fail/queue safely. |
| Gateway/node/WG restart | Last acknowledged state returns without manual rule injection. |
| Bad state | Invalid staged rule/config rolls back or never replaces known-good state. |
| Isolation/quarantine | Scoped withdrawal blocks only the intended server/node and preserves management. |

## Packet-capture assertion

For every transparent-routing acceptance test, capture at gateway WAN/wg0, node wg0/bridge, and endpoint. Assert:

* Forward source: `client_ip` at all stages.
* Gateway forward destination: `node_tunnel_ip:backend_port`.
* Node forward destination: `container_ip:container_port`.
* Node reply egress: `wg0` with a restored connection mark/policy route.
* Gateway reply egress: `public_ip:public_port` to `client_ip`.

A WireGuard handshake, a successful ping, or a local host socket check is supporting telemetry only; none is a transparent-routing acceptance result.

## Current test result

At the inspected revision, only one Rust unit test exists and it covers the old TCP-proxy rate limiter. During this documentation pass, CI working-directory wiring and behavior-neutral formatting/lint issues were corrected: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace --locked` pass. No Linux/network/container/Pterodactyl integration test exists, so every matrix row above remains unverified until the replacement implementation is built and exercised.
