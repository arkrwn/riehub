# ADR 0012: Stateless nodes over a shared database, no gossip cluster

Status: accepted

## Context

riehub must run as one process on a small VM and as several replicas
on Kubernetes. The enterprise gateway it draws from uses a gossip
cluster to share state between nodes.

## Decision

- A node keeps no durable state. Everything durable is in the store.
- Sessions and hub keys unlock vaults from the presented token alone
  (ADR 0005), so there is no session affinity.
- Limits, budgets and quota windows are atomic counters in the store.
  The audit chain and scheduler leases are in the store.
- Provider health, circuit breakers, sticky conversation keys and the
  unlock cache are per node, in memory, and safe to lose.
- Several nodes need PostgreSQL or MongoDB. SQLite is single node and
  a second node is refused.

## Alternatives considered

- **Gossip or Raft between nodes:** a second distributed system to
  secure and operate, for state the database can hold.
- **A cache service (Redis) for counters:** another network hop and
  another place that must never see content. Can be added behind the
  counter interface if the database becomes the bottleneck.

## Consequences

Every request costs a few database round trips. Counter updates are
the scaling limit. Health data differs slightly between nodes, which
is acceptable for routing.
