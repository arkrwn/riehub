# MCP

Per-user connectors and the MCP gateway. Crate: `riehub-mcp`. Decision:
ADR 0009.

## 1. Concepts

- A **catalog entry** describes an MCP server or integration: name,
  icon, URL, transport, auth type, and for OAuth the endpoints or the
  discovery URL. The catalog is instance data. riehub ships a starter
  catalog. Admins curate it. It holds no credentials.
- A **connector** is one user's connection to an MCP server: a catalog
  entry, or a custom URL, plus that user's credential in their vault.
  Two users of the same integration have two connectors with two
  accounts.
- The **gateway** is riehub acting as one MCP server per user, in
  front of all of that user's connectors.

## 2. Connecting

| Transport | Since |
|---|---|
| Streamable HTTP | 0.4.0 |
| HTTP with SSE (older servers) | 0.4.0 |
| stdio, local process in a sandbox | 0.9.0, off by default |

| Auth | Notes |
|---|---|
| none | |
| header | a static header such as a bearer token or API key, in the vault |
| OAuth 2.1 | authorization code with PKCE. Uses protected resource metadata and authorization server metadata discovery, and dynamic client registration when the server offers it. Tokens live in the user's vault and are refreshed inside requests |

The OAuth callback is `GET /api/v1/oauth/callback`. The `state` value
is random, single use, bound to the session, and expires after 10
minutes. Every MCP URL goes through the guarded HTTP client (S11).

A connector has a status: `connected`, `needs_auth`, `error`. The tool
list of a connector is cached as an encrypted record and refreshed on
demand and on `tools/list_changed`.

## 3. The gateway

| Route | Serves |
|---|---|
| `POST /mcp`, `GET /mcp` | every connector of the hub key's user |
| `/mcp/<slug>` | one virtual MCP |

Auth is a hub key with the `mcp` scope. Transport is Streamable HTTP.

- **Names**: tools are exposed as `<connector-slug>__<tool>`. Resources
  and prompts are prefixed the same way.
- **Filtering**: the tools a caller sees and may call are the
  intersection of: tools the user enabled on the connector, the hub
  key's `tools` allowlist, the virtual MCP's tool list, and the access
  profile. A call to a tool outside the set gets "tool not found".
- **Calls**: forwarded with the user's credential for that connector.
  Arguments and results are `Content`. The audit event `mcp.tool_call`
  holds the connector id, the tool's position in the connector's cached
  tool list, the outcome and the duration. It never holds the tool name
  as sent by the caller, the arguments or the result.
- **Limits**: per-call timeout 60 seconds by default, result size 8 MiB,
  and the hub key's rate limit.

A **virtual MCP** is a named bundle of tools from several connectors,
owned by a user, with its own slug. Admins can publish bundles of
catalog entries through access profiles. Each user still connects their
own accounts.

## 4. Tools inside the router

A router request can use the user's MCP tools without the client
speaking MCP:

- **Tool injection**: with the header `x-riehub-tools: <virtual-mcp-slug
  or "all">`, the gateway's tool definitions are added to the request.
- **Agent mode**: with `x-riehub-agent: auto`, riehub executes tool
  calls itself and continues the model loop, for tools on the hub key's
  auto-approve list only. Other tool calls are returned to the client.
  At most 10 steps per request by default.

Tool results are data. They never change the hub key's permissions,
and a tool result cannot add tools to the allowlist.

## 5. Built-in tools (version 0.6.0)

riehub exposes two tools of its own through the gateway:
`web_search` (the user supplies a search API key, stored at
`tool/<name>/key`) and `web_fetch` (through the guarded client, with a
size limit and text extraction).

## 6. Local stdio servers (version 0.9.0)

Off unless an admin sets `mcp.stdio = true`. Linux only. The process
runs under `bwrap` with no network unless the connector asks for it,
a private temp directory, no access to the data directory, a memory
and CPU limit, and is killed after an idle timeout. The command comes
from a catalog entry an admin approved, never from free text a user
typed.
