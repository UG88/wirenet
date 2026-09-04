# WireNet Networking Reference

This reference defines the initial **IPv4-only** transparent-routing implementation. It is not a copy-and-paste firewall script; the controller must render values from validated desired state.

## Addressing

* Give every gateway and node a unique, non-overlapping tunnel address. Example: gateway `10.100.0.1/24`, nodes `10.100.0.2/32`, `10.100.0.3/32`, and so on.
* Public IPv4 addresses belong to/routably terminate at the gateway. They are never assigned to backend nodes solely for game ingress.
* Container address ranges are discovered and recorded per node; they must not overlap the WireNet tunnel or management ranges.
* A Pterodactyl server is identified by its server UUID and WireNet `server_id`, not by an ephemeral container IP.

## Mapping semantics

One active mapping is:

```text
(gateway, public IPv4, public port, protocol)
    -> (server ID, node ID, node tunnel IPv4, backend port, container endpoint)
```

The active key is unique. TCP and UDP are distinct protocol keys. A backend port need not equal its public port. Mapping maps, counters, and firewall policies must carry the mapping/server identity.

## Packet and route invariants

1. The gateway DNATs only the destination. It never source-NATs WireGuard game traffic.
2. Node ingress from `wg0` marks the connection and stores the mark in conntrack. Reply packets restore that mark before their route lookup.
3. An `ip rule` selects a WireNet-only routing table for that mark. That table sends matching traffic through `wg0`; the node's main default route remains for normal host management traffic.
4. The node reverses its local/container DNAT before encapsulating the reply. The gateway conntrack entry reverses gateway DNAT on the final reply to the player.
5. WireGuard peer prefixes are minimal and non-overlapping. Gateway peer selection must select exactly the intended node for each node tunnel destination.
6. No public game socket is owned by a WireNet userspace process.

## Firewall design

Gateway rules are keyed by public destination address and protocol/port, then DNAT to a node tunnel address/backend port. Node rules are keyed by the known node tunnel destination and backend port, then DNAT to the current endpoint for the owning server. Both roles count accepted/dropped traffic by mapping and use conntrack state for replies.

Default policies deny new traffic that does not match a mapping. This includes unallocated ports, direct node-provider traffic, cross-node tunnel traffic, and cross-customer forwarding. Temporary bans and rate limits are scoped to the destination mapping or policy target; they do not become host-wide anonymous iptables additions.

## Docker/Pterodactyl contract

The node agent must know the current container address and port for a Pterodactyl allocation. It must withdraw an endpoint before it becomes stale and only route to an endpoint that it can associate with the expected server ID.

Docker's firewall backend is discovered at enrollment. The supported backend/version set is explicitly tested. Docker tables/chains are Docker-owned; WireNet uses its own nftables table and the appropriate tested integration point. A configuration that cannot guarantee direct container forwarding while preserving source IP is unsupported, not patched with a proxy protocol or a blanket accept rule.

## IPv6

IPv6 is not implemented in the initial release. The controller must report this clearly and must not allocate or advertise IPv6 mappings. IPv6 work requires parallel public-pool validation, WireGuard peer policy, `ip6`/`inet` firewall rules, return routing, and end-to-end source-preservation tests.

## Diagnostic capture points

For an authorized diagnostic, capture headers only and time-bound the collection at:

```text
client test namespace
gateway WAN ingress
gateway wg0
node wg0
node Docker bridge
container interface / application log
gateway WAN egress (reply)
```

Expected forward source is the client public/test address at every capture point. Expected destination changes at gateway DNAT and node DNAT only. The reply makes the inverse transformations.
