# ADR 0009: MCP connectors are remote first, local processes are sandboxed and opt in

Status: accepted

## Context

MCP servers come in two shapes: remote HTTP servers, and local
programs started over stdio. On a shared server, a stdio connector is
a way for a user to run a program on the host.

## Decision

- Connectors use Streamable HTTP or HTTP with SSE by default, through
  the guarded HTTP client.
- stdio connectors exist from version 0.9.0, are off by default, need
  `mcp.stdio = true`, run on Linux only inside a `bwrap` sandbox with
  no data directory access, and can only start commands from catalog
  entries an admin approved.
- riehub is one MCP server per user (the gateway). Tool visibility is
  the intersection of every allowlist that applies.
- Tool results are untrusted data. They cannot change permissions.

## Alternatives considered

- **stdio everywhere, like desktop clients:** remote code execution by
  design on a multi-user host.
- **No stdio at all:** many useful servers exist only as local
  programs. A sandbox with an admin-approved catalog keeps the door
  narrow.

## Consequences

Out of the box only remote MCP servers work. macOS hosts never run
stdio connectors. Users who need a local-only server run it on their
own machine and reach it some other way.
