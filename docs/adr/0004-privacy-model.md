# ADR 0004: Operators manage metadata, users hold the keys

Status: accepted

## Context

The requirement is that not even the super admin can read what passes
through the router, while an audit log still exists. A router has to
see a prompt in memory to forward it, so "cannot read" needs a precise
meaning that can be tested.

## Decision

1. **At rest, user data is unreadable without the user.** Credentials
   and content are encrypted under keys that derive from a secret the
   user presents with each request (ADR 0005). The database, backups
   and the data directory contain ciphertext only.
2. **No interface returns it.** No route, command, export or role
   exposes one user's content or secrets to another principal. Roles
   are sets of metadata permissions. The route table makes this
   testable for every route (spec, API, section 2).
3. **Nothing records it.** Logs, metrics, traces, errors and the audit
   log carry allowlisted metadata only. Types (`Secret<T>`, `Content`)
   and a canary test enforce it.
4. **The audit log is about actions, not content.** Who did what, to
   which object, when, with what outcome, and for AI requests the
   model and sizes. It is hash chained with a key the database does not
   hold (ADR 0010).
5. **The limit is stated, not hidden.** Someone with root on the host
   can inspect process memory while a request runs. riehub hardens
   against this and does not claim to prevent it.

## Alternatives considered

- **Client-side encryption end to end:** impossible for a router, the
  provider needs the plain prompt.
- **A master key held by the server:** simple, and it makes every
  admin and every backup a way to read everything.
- **Content logging with access control:** access control is a policy,
  and policies get changed. Not storing it cannot be changed quietly.
- **Confidential computing (enclaves):** would narrow limit 5. Left for
  after 1.0 as a deployment option, it does not change this design.

## Consequences

Features that need content on the server while the user is away are
either impossible or need an explicit, visible grant (ADR 0013).
Password recovery needs the user's recovery key. An admin cannot help
a user who lost both. Support and debugging work from metadata.
