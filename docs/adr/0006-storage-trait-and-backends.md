# ADR 0006: One record-shaped store trait, four databases

Status: accepted

## Context

riehub must run standalone with no database server, and in clusters.
The allowed databases are SQLite, PostgreSQL, MongoDB and Oracle
(lakehouse or JSON). Most user data is opaque ciphertext, so rich
queries over it are neither possible nor wanted.

## Decision

- All persistence goes through the `Store` trait in `riehub-store`
  (spec, storage). Data is a generic **record** with a small fixed set
  of filterable fields and an opaque `data` blob, plus four purpose
  built areas: the audit chain, counters, usage, leases.
- No SQL or driver call exists outside `riehub-store`.
- SQLite is the default and ships first. PostgreSQL follows for
  several nodes. MongoDB is a second-tier backend.
- Oracle is reached through the Oracle Database API for MongoDB, with
  the MongoDB backend. There is no separate Oracle driver, which would
  need a native client library.
- One conformance suite runs against every backend.

## Alternatives considered

- **An ORM with a relational schema per feature:** four dialects of
  every migration, and MongoDB would not fit.
- **PostgreSQL only:** breaks the standalone requirement.
- **A native Oracle backend:** needs Oracle's C client at build and
  run time, which complicates a static, auditable binary.

## Consequences

Queries are limited to what the trait offers. Reports are built from
the usage area, not from ad hoc SQL. Adding a backend is one module
plus passing the suite. Oracle support is "expected to work" until
someone runs the suite against a real instance.
