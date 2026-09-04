# WireNet

WireNet is being rebuilt as a transparent, kernel-forwarded IPv4 ingress system for private Pterodactyl/Docker backends. The public gateway owns public IPs and applies destination NAT; WireGuard encrypts traffic to private nodes; node policy routing returns reply traffic through the gateway; applications retain the real client source address. No customer plugin, proxy protocol, or special client configuration belongs in the normal path.

## Deployment status

The inspected Rust implementation is **not approved for production deployment**. It contains an incompatible userspace TCP forwarding path and unsafe mutable networking behavior. Do not run its legacy `setup`, `apply`, `doctor`, `update`, or historical shell instructions against a production host.

The reconstruction is governed by these documents:

* [Current architecture inventory](CURRENT_ARCHITECTURE.md)
* [Gap analysis and KEEP/MODIFY/REWRITE decisions](WIRENET_GAP_ANALYSIS.md)
* [Recommended architecture](WIRENET_ARCHITECTURE.md)
* [Migration plan](WIRENET_MIGRATION.md)
* [Networking reference](WIRENET_NETWORKING.md)
* [Security model](WIRENET_SECURITY.md)
* [Operations reference](WIRENET_OPERATIONS.md)
* [Test plan](WIRENET_TESTING.md)
* [Authoritative specification](WIRENET_SPEC.ai)

The next implementation milestone is a testable desired-state controller/agent and nftables-based gateway/node renderer, proven in a Linux namespace + Docker/Pterodactyl-compatible integration topology before any production migration.
