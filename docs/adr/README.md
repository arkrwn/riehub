# Architecture decisions

[`SPEC.md`](../SPEC.md) is the source of truth for product behavior and
security requirements. ADRs briefly record why a consequential design
choice was made.

Write an ADR when a decision changes a security boundary, storage
format, public interface, supported platform, or operating model. Keep
it short: context, decision, and main tradeoff. Routine implementation
choices do not need an ADR or approval. Existing ADRs are decision
history and do not prescribe a delivery process.

| ADRs | Topic |
|---|---|
| [0001](0001-record-architecture-decisions.md), [0002](0002-rust-single-binary-linux-macos.md), [0003](0003-software-factory.md) | Project structure and delivery |
| [0004](0004-privacy-model.md), [0005](0005-user-vault-on-cofferak.md), [0010](0010-audit-log.md) | Privacy, vaults, and audit |
| [0006](0006-storage-trait-and-backends.md), [0012](0012-stateless-nodes.md) | Storage and scale |
| [0007](0007-canonical-request-model.md), [0008](0008-connection-auth-types.md) | Router and provider connections |
| [0009](0009-mcp-remote-first.md), [0013](0013-standing-grants.md), [0014](0014-wasm-plugins.md) | MCP and automation |
| [0011](0011-web-ui-stack.md), [0015](0015-ports-tls-and-tunnel.md) | Web UI and deployment |
