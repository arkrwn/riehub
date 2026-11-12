# ADR 0008: Connection auth types, and where sign-in is allowed

Status: accepted

## Context

Users want to connect providers "by OAuth or by API key, whatever the
provider offers". Some desktop tools also reuse consumer subscription
logins by acting as the vendor's own client. On a shared server that
is a different thing: it can break the provider's terms and get every
user's account closed.

## Decision

- Four auth types: `api_key`, `none`, `oauth2`, `cloud` (spec, router,
  section 4). Every credential lives in the owning user's vault.
- The OAuth engine is generic: authorization code with PKCE, device
  code, refresh. A provider gets OAuth sign-in through a catalog entry
  that names its endpoints and its registered client.
- A catalog entry for sign-in is added only when the provider's terms
  allow third-party clients. The entry records the link to those terms
  and the date it was checked.
- riehub never presents itself as a vendor's official client, never
  ships a vendor's client id or secret that was not issued to the
  instance, and never reads another tool's stored login.

## Alternatives considered

- **Reuse local agent logins like desktop managers do:** works on one
  person's machine with their own account. Not acceptable for a
  multi-user server.
- **API keys only:** misses providers whose only programmatic access is
  OAuth or cloud IAM.

## Consequences

Some consumer subscriptions cannot be connected. Users of those
providers connect with an API key instead. Admins can add catalog
entries for providers that permit OAuth clients, with their own
registered client.
