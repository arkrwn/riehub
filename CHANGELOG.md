# Changelog

All notable changes to riehub are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- React web app shell, the design token system and reusable account
  components for setup, login, recovery and invite acceptance. The UI
  is embedded in the server and the API types come from OpenAPI.
- Account self-service in `riehub-identity` and the API: password
  change with re-wrapping, recovery through the armored key, recovery
  key rotation, and the admin reset with recovery-or-start-empty.
- Hub keys in `riehub-identity` and the API: the `rh_<ID>_<secret>`
  token shown once, the three accepted headers, scopes and ACL
  prefixes, encrypted labels, the Key route class, and manage keys on
  Self routes without key creation.
- Roles, route classes and user administration in `riehub-identity`
  and the API: the owner/admin/member roles with the permission enum,
  the class middleware driven by the route table, the admin user
  routes with the last-owner protection, one-time invites with the
  72-hour expiry, and the `UserScope` built only from an `AuthUser`.
- Web sessions in `riehub-identity` and the API: the `rs_<SID>_<secret>`
  token and cookie, CSRF enforcement on cookie-carrying requests, idle
  and absolute timeouts, the ten-session cap, the in-memory unlock
  cache, and the pw slot and verifier derivations from the spec table.
- Users, setup and password login in `riehub-identity` and the API:
  the owner setup on an empty instance, Argon2id login with dummy
  work for unknown emails, a four-way Argon2 semaphore, store-backed
  throttling, the embedded common-password list, and the armored
  recovery key shown once.
- The instance coffer in `riehub-vault` and the server: host key
  seals behind `instance.seal` (systemd, keychain, file), the audit
  and cookie keys stored inside it, `riehub init` printing the
  armored recovery key once, and a doctor seal check.
- The per-user vault in `riehub-vault` on cofferak-core: unlock
  slots from secrets the user presents, recovery keys, ACL-scoped
  entries, content keys for encrypted records, generation compare-and-swap
  with the `vault.write` audit event, and rollback refusal.

## [0.0.1] - 2026-11-12

### Added

- The HTTP server skeleton on axum: the static route table with
  classes, middleware (request id, limits, timeout, security headers,
  allowlisted access log), problem json errors, process hardening,
  `riehub init`, graceful `riehub serve` with the `SystemStart` audit
  event, and tracing JSON output.
- The audit log in `riehub-audit` and the CLI: typed events without
  content fields, an HMAC-SHA384 chain over the store, 10 ms batching,
  verification with tamper detection, plus `riehub audit
  verify|head|export`.
- The `Store` trait and the SQLite backend in `riehub-store`: records,
  audit chain, counters, usage, leases, migrations, file mode 0600, and
  the conformance suite behind the `conformance` feature.
- Crypto primitives in `riehub-crypto` on aws-lc-rs and argon2:
  AES-256-GCM-SIV AEAD, HKDF-SHA384 with compile-time checked info
  prefixes, Argon2id, HMAC-SHA384, SHA-384, random bytes and
  constant-time compare.
- Configuration loading in `riehub-core` and the binary: the spec TOML
  with defaults, `RIEHUB_*` env overrides, profile validation, plus
  `riehub serve --config` and `riehub doctor`.
- Core types in `riehub-core`: typed ids (`<prefix>_<ULID>`), `Secret<T>`
  and `Content` that never leak through `Debug`, `Display` or `Serialize`,
  the eight-class `Error`, `Clock` with a test double, and `Owner`.
- Project skeleton: SDD practice, AGENTS.md, ADR log, spec and progress
  table, MkDocs Material documentation site with GitHub Pages deploy, CI.
- The design: specification with security invariants S1 to S12, ten
  subsystem pages, ADRs 0002 to 0015, and the roadmap to 1.0.0.
- The factory: task cards for versions 0.0.1 to 1.0.0, the
  `factory/factory.sh` driver, the gate (`make gate`) and the project
  rules lint.

[Unreleased]: https://github.com/arkrwn/riehub/commits/main
[0.0.1]: https://github.com/arkrwn/riehub/commits/main
