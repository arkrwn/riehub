# API and configuration

HTTP conventions, the route table, the config file and the two
binaries. Crates: `riehub-server`, `riehub-cli`.

## 1. HTTP conventions

| Area | Prefix | Credential |
|---|---|---|
| Management API | `/api/v1` | session cookie with CSRF header, or a hub key with scope `manage` |
| AI surfaces | `/v1`, `/v1beta` | hub key with scope `ai` ([router](router.md)) |
| MCP gateway | `/mcp` | hub key with scope `mcp` ([MCP](mcp.md)) |
| Webhooks | `/hooks` | request signature ([automation](automation.md)) |
| Health | `/healthz`, `/readyz` | none |
| Metrics | `/metrics` | separate listener, see section 3 |
| Web UI | everything else | static files embedded in the binary |

Management API rules:

- JSON in and out, `snake_case` keys, UTC timestamps in RFC 3339.
- Errors are `application/problem+json`: `type`, `title`, `status`,
  `detail`, `request_id`. `detail` never echoes request content.
- Lists take `limit` (default 50, cap 200) and `cursor`, and return
  `items` and `next_cursor`.
- Updates of a record send `If-Match: <version>` and get `409` on a
  conflict.
- A resource that belongs to another user answers `404`, never `403`,
  so ids cannot be probed.
- Every response carries `x-request-id`.
- The OpenAPI document is generated from the code by `riehub openapi`
  and committed as `docs/api/openapi.json`. `make check` fails when it
  is stale. The web UI's types are generated from it.

## 2. The route table (S3, S5)

`crates/riehub-server/src/routes.rs` holds one static table. The HTTP
application is built from it and from nothing else.

```text
RouteSpec { method, path, class, permission: Option<Permission>, handler }
class = Public | Self_ | AdminMeta | Key(Scope) | Hook
```

| Class | Meaning | Test generated for every route of the class |
|---|---|---|
| `Public` | no credential (health, login, setup, static files) | listed in an explicit allowlist test |
| `Self_` | the caller's own resources | no credential gets `401`. Another user's id gets `404`, also for an owner |
| `AdminMeta` | instance metadata, needs `permission` | a member gets `403`. The response type contains no `Content` or `Secret` field (compile-time marker trait) |
| `Key(scope)` | hub key surfaces | no key gets the surface's auth error. A key without the scope gets `403` |
| `Hook` | signed webhooks | a bad signature gets `401` |

Adding a route without a class does not compile. The class tests
iterate the table, so a new route is covered the moment it is added.

Management resources (all under `/api/v1`):

| Resource | Class | Since |
|---|---|---|
| `setup`, `auth/login`, `auth/logout`, `auth/recover`, `invites/{token}` | Public | 0.1.0 |
| `me`, `me/password`, `me/sessions`, `me/keys`, `me/security` | Self | 0.1.0 |
| `connections`, `models`, `usage` | Self | 0.2.0 |
| `routing-groups`, `limits` | Self | 0.3.0 |
| `connectors`, `virtual-mcps`, `oauth/start`, `oauth/callback` | Self | 0.4.0 |
| `skills`, `instructions`, `profiles`, `chats`, `workflows`, `runs`, `grants` | Self | 0.5.0 |
| `privacy/rules`, `privacy/log` | Self | 0.6.0 |
| `admin/users`, `admin/invites`, `admin/audit` | AdminMeta | 0.1.0 |
| `admin/catalog/providers`, `admin/usage` | AdminMeta | 0.2.0 |
| `admin/teams`, `admin/profiles`, `admin/shared-connections` | AdminMeta | 0.3.0 |
| `admin/catalog/connectors` | AdminMeta | 0.4.0 |
| `admin/roles`, `admin/sso`, `admin/exports`, `scim/v2/*` | AdminMeta | 0.7.0 |

## 3. Configuration

One TOML file (`--config`, default `./riehub.toml`, then
`/etc/riehub/riehub.toml`). Every key can be set by environment
variable: `RIEHUB_` plus the path in upper case with `__` between
levels, for example `RIEHUB_SERVER__BIND`. Secrets are never put in
this file. They go in the instance coffer.

```toml
profile = "dev"                      # dev | production

[server]
bind = "127.0.0.1:8080"              # dev default. production default "0.0.0.0:443"
public_url = "http://127.0.0.1:8080" # for example "https://hub.rie.my.id"
trusted_proxies = []                 # CIDRs allowed to set the client IP header
client_ip_header = ""                # for example "CF-Connecting-IP"
metrics_bind = ""                    # empty disables /metrics, for example "127.0.0.1:9090"
max_body_bytes = 33554432

[server.tls]
mode = "off"                         # off | files
cert = ""                            # PEM chain
key = ""                             # PEM key, mode 0600

[data]
dir = "./data"

[database]
url = "sqlite://./data/riehub.db"

[instance]
seal = "systemd"                     # systemd | keychain | file
key_file = ""                        # for seal = "file"

[identity]
signup = "invite"                    # invite | open | closed

[audit]
ip = "truncated"                     # truncated | full | off
retention_days = 400

[egress]
allow = []                           # hosts or CIDRs exempt from the SSRF guard, for example ["127.0.0.1:11434"]

[mcp]
stdio = false

[log]
level = "info"
format = "json"                      # json | text
```

Profile rules, checked at start:

- `production` refuses to start when `server.tls.mode = "off"` unless
  `server.trusted_proxies` is not empty (a TLS-terminating proxy is in
  front), when `public_url` is not `https`, or when the data directory
  is readable by other users.
- `dev` binds to loopback only. Binding `dev` to another address is an
  error.

## 4. The `riehub` binary

| Command | Does |
|---|---|
| `riehub init` | creates the data directory, the store schema, the instance coffer, prints the instance recovery key once |
| `riehub serve` | runs the server. `--migrate` applies pending migrations first |
| `riehub migrate` | applies pending migrations |
| `riehub audit verify`, `audit head`, `audit export` | checks and exports the audit chain |
| `riehub backup <file>`, `riehub restore <file>` | see [storage](storage.md) |
| `riehub doctor` | checks config, file modes, seal, store, clock, TLS files |
| `riehub openapi` | prints the OpenAPI document |
| `riehub version` | prints the version and the build commit |

Exit codes: `0` ok, `1` error, `2` usage, `3` verification failed.

## 5. The `rie` CLI (version 0.9.0)

`rie` runs on a user's Linux or macOS machine and talks to a hub with
a hub key kept in the OS keychain or a `0600` file.

| Command | Does |
|---|---|
| `rie login <hub-url>` | device-style login: shows a code, the user approves in the web UI, the CLI receives a hub key |
| `rie status` | hub, user, key scopes, quota |
| `rie agents` | lists coding agents found on the machine |
| `rie use <agent> <model>` | points an agent at the hub with a model or routing group, editing only the needed keys of its config file, keeping comments and formatting, with a backup |
| `rie undo <agent>` | restores the last backup |
| `rie profile save or apply <name>` | saves and applies a set of agent settings |
| `rie sync` | writes the user's instructions, skills, agent profiles and MCP gateway entry into each agent's own format |
| `rie import <source>` | imports provider keys and settings from another tool's export into the hub |

Supported agents start with Claude Code, Codex, Gemini CLI and
OpenCode. Each agent is one module that knows its config path and
format. Config writes are atomic (temp file, fsync, rename).
