# WireNet Troubleshooting

The historical troubleshooting guidance referenced deleted scripts and fallback proxies. It is intentionally retired: HAProxy, rinetd, PROXY Protocol, a Minecraft plugin, a firewall flush, or a blanket forwarding accept are not fixes for a transparent-routing deployment.

Until the replacement implementation is complete, do not mutate a live gateway/node with legacy WireNet commands. Preserve the current state, collect a read-only snapshot, and follow the stop conditions in [WIRENET_MIGRATION.md](WIRENET_MIGRATION.md).

The target diagnostic behavior is defined in [WIRENET_OPERATIONS.md](WIRENET_OPERATIONS.md) and [WIRENET_TESTING.md](WIRENET_TESTING.md): prove the full packet path at gateway WAN/wg0, node wg0/bridge, and the backend endpoint. A WireGuard handshake or local socket test alone is not sufficient.
