# Architecture decision records

One decision per file, numbered in the order they were made. ADRs are
history: when a later ADR changes part of an earlier one, the earlier
file keeps its text and gets an "Amended by ADR NNNN" note under its
Status line. Conventions are in
[`SDD.md`](https://github.com/arkrwn/riehub/blob/main/SDD.md#adr-conventions),
start a new one from [`template.md`](template.md).

| ADR | Title | Status | Summary |
|---|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | accepted, amended by ADR 0003 | Design decisions are recorded as numbered ADRs in `docs/adr/`, alongside a spec and a progress table. |
| [0002](0002-rust-single-binary-linux-macos.md) | Rust, one binary, Linux and macOS only | accepted | Everything but the web UI is Rust with `unsafe` forbidden, shipped as one binary. Windows is never a target. |
| [0003](0003-software-factory.md) | A software factory with a gate-owned progress record | accepted | Fixed task cards, one branch per task, and a script that records `done` only on a green gate. |
| [0004](0004-privacy-model.md) | Operators manage metadata, users hold the keys | accepted | What "the admin cannot read it" means, how it is enforced, and where the limit is. |
| [0005](0005-user-vault-on-cofferak.md) | Per-user vaults built on cofferak-core | accepted | One coffer per user, with unlock slots keyed by secrets the user presents. |
| [0006](0006-storage-trait-and-backends.md) | One record-shaped store trait, four databases | accepted | SQLite and PostgreSQL first, MongoDB second, Oracle through its MongoDB API. |
| [0007](0007-canonical-request-model.md) | A canonical request model between surfaces and providers | accepted | Four API formats decode into one model that every provider adapter encodes from. |
| [0008](0008-connection-auth-types.md) | Connection auth types, and where sign-in is allowed | accepted | API key, none, OAuth and cloud credentials. No posing as a vendor's own client. |
| [0009](0009-mcp-remote-first.md) | MCP connectors are remote first | accepted | Remote MCP by default. Local stdio servers are sandboxed, Linux only and opt in. |
| [0010](0010-audit-log.md) | A typed, keyed, hash-chained audit log without content | accepted | Events hold ids and enums only and are chained with a key the database does not hold. |
| [0011](0011-web-ui-stack.md) | Web UI stack and design system | accepted | React, TypeScript, Vite and Tailwind, embedded in the binary, nothing from other origins. |
| [0012](0012-stateless-nodes.md) | Stateless nodes over a shared database | accepted | No gossip cluster. Tokens unlock vaults on any node, counters live in the store. |
| [0013](0013-standing-grants.md) | Unattended runs need an explicit standing grant | accepted | A scheduled workflow runs only with a visible, scoped, revocable grant from its owner. |
| [0014](0014-wasm-plugins.md) | Plugins are WebAssembly with explicit capabilities | accepted | No native or scripted plugins. No files, and network only to approved hosts. |
| [0015](0015-ports-tls-and-tunnel.md) | Port 8080 for development, 443 with native TLS for production | accepted | riehub terminates TLS itself in production. The tunnel is for testing. |
