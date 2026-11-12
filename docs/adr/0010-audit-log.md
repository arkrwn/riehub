# ADR 0010: A typed, keyed, hash-chained audit log without content

Status: accepted

## Context

Privacy forbids recording content. Accountability needs a record that
an operator cannot quietly edit. Both must hold at once.

## Decision

- Audit events are variants of one Rust enum. A variant may hold ids,
  enums, numbers, booleans and timestamps. It may not hold a string or
  bytes that came from a request.
- Each event's `mac` is `HMAC-SHA384(audit_key, prev_mac || event)`.
  The audit key is in the instance coffer, not in the database, so
  someone who can write the database cannot forge or re-chain events.
- Appends go through the store in one serialized chain with gapless
  sequence numbers.
- An AI request writes its event before the provider is called. If the
  write fails, the request is refused.
- `riehub audit verify` checks the chain. `riehub audit head` prints
  the head for anchoring outside the system.
- Client IP addresses are truncated by default.

## Alternatives considered

- **Plain rows:** editable without trace.
- **An unkeyed hash chain:** anyone who can rewrite the table can
  recompute it.
- **Signing each event with an asymmetric key:** verification without
  the secret, at a per-event cost. Can be added as an export format.
- **One chain per node:** less contention, but gaps between chains
  hide deletions.

## Consequences

Audit writes are a serialized hot path. Batching within 10 ms keeps
throughput acceptable. Verification needs the instance key. Whoever
holds root on the host can read that key, which is inside the stated
limit of ADR 0004, and is why the head should be anchored elsewhere.
