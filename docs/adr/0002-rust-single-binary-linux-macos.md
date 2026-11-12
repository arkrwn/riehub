# ADR 0002: Rust, one binary, Linux and macOS only

Status: accepted

## Context

riehub handles every user's credentials and prompts. Memory safety and
a small, auditable deployment matter more than how fast a feature can
be written. The maintainer has ruled out Windows for good.

## Decision

- Everything except the web UI is Rust, in one Cargo workspace under
  `crates/`. Every riehub crate starts with `#![forbid(unsafe_code)]`.
  Code that needs `unsafe` lives in a dependency that is reviewed for
  that purpose (`aws-lc-rs`, `nix`, `cofferak-core`).
- The server is one binary, `riehub`, with the web UI embedded. The
  client is a second binary, `rie`.
- Supported targets: Linux x86-64 and arm64 (Ubuntu 24.04 LTS or newer,
  or equivalent), macOS arm64 and x86-64. CI runs the gate on Linux and
  macOS.
- Windows is never a target. No `cfg(windows)` branch exists.
  `scripts/rules-lint.sh` rejects one.
- The async runtime is `tokio`. HTTP is `axum` on `hyper`. TLS is
  `rustls` with `aws-lc-rs`. SQL is `sqlx`. These are the base
  dependencies. Others are named by the spec or a task card.

## Alternatives considered

- **Go**, like both reference projects: faster to write, but no
  compile-time control over who may hold a secret, and the vault
  (cofferak) is Rust.
- **Separate services** for router, MCP and UI: more deployment
  surface and more places for secrets to cross a network.

## Consequences

Builds are slower and contributors need Rust. In return there is one
artifact to sign, scan and run, and the type system carries part of the
privacy model (`Secret<T>`, `Content`, `UserScope`).
