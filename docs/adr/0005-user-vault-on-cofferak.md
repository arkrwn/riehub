# ADR 0005: Per-user vaults built on cofferak-core

Status: accepted

## Context

Each user needs a vault for provider keys and tokens, and a key
hierarchy for their content. The maintainer requires
[cofferak](https://github.com/arkrwn/cofferak) for it. cofferak is a
single-root coffer: one data key, wrapped for a recovery key and for
each enrolled host, with the host key sealed by the operating system.
Used as is, the host could open every user's coffer, which breaks
ADR 0004.

## Decision

- riehub embeds the library crate `cofferak-core` (git dependency,
  pinned by `rev`). It does not run the `cofferakd` daemon.
- **One coffer per user**, a `VaultFile` stored as a record.
- cofferak's "host" slots are used as **unlock slots**. The 32-byte
  key that cofferak expects from a host seal comes instead from a
  secret the user presents: password, session token, hub key, passkey.
  One slot per credential, added and removed with the credential.
- cofferak's root keypair is the user's **recovery key**, shown once.
- Credentials are coffer entries. Bulk content is stored as records
  encrypted under keys derived from the coffer's data key with HKDF.
- Hub key scoping uses cofferak's path-prefix ACL.
- Instance secrets use a second coffer the normal cofferak way, sealed
  by `SystemdCredsSeal`, `KeychainSeal`, or a key file in containers.

## Alternatives considered

- **Run cofferakd per user:** one process and one sealed host key per
  user. The host could still unseal all of them.
- **A new vault format:** more code to get wrong, and no shared
  recovery story with cofferak.
- **Per-entry keys per hub key (cryptographic scoping):** stronger than
  an ACL check, at the cost of re-wrapping on every change. Possible
  later inside the same format.

## Proof

`cofferak-core` at commit `ad45409` exposes what this needs as public
API: `format::VaultFile` (`new`, `seal`, `verify`, `to_bytes`,
`from_bytes`), `crypto::{generate_root_keypair, wrap_dek_for_root,
unwrap_dek_for_root, wrap_dek_for_host, unwrap_dek_for_host,
encrypt_entry, decrypt_entry, Aad::entry}`, `acl::path_allowed`, and
the `seal::HostSeal` trait. Task T-0101 pins it with tests.

## Consequences

A hub key or session token opens the whole user vault in memory for
the length of a request. Scope is enforced by the ACL in code. A
cofferak format change means a pinned-revision bump task. riehub gains
cofferak's post-quantum recovery wrap and integrity checks for free.
