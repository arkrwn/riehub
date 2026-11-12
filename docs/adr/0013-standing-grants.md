# ADR 0013: Unattended runs need an explicit standing grant

Status: accepted

## Context

Scheduled and webhook-triggered workflows run when the user is not
there. ADR 0004 says the server cannot open a vault without a secret
the user presents. Something has to give, and it must be the user who
decides.

## Decision

- A workflow can run unattended only if its owner created a **standing
  grant** for it from a web session, after reading a plain warning.
- A grant is an unlock slot on the user's vault whose key is kept in
  the instance coffer. Its ACL covers only the connections and
  connectors that workflow uses.
- Grants are listed on the user's security page, recorded in the audit
  log when created, used and revoked, and removed automatically when
  the workflow's tools change.
- Chats never get a grant. They exist only while a session is open.

## Alternatives considered

- **No unattended runs:** removes a core use of workflows.
- **A server master key:** gives the host every vault, all the time.
- **A long-lived hub key stored by the scheduler:** the same exposure
  as a grant, without the visibility, scoping and audit.

## Consequences

For a user with grants, the host can act as the granted workflows. The
threat model says so. Users without grants keep the full guarantee.
