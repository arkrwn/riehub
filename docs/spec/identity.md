# Identity

Users, login, sessions, roles and hub keys. Crate: `riehub-identity`.
The key handling it relies on is in [security](security.md).

## 1. Users

| Field | Notes |
|---|---|
| `id` | `usr_<ULID>` |
| `email` | unique, lower case, plain (admins manage users by it) |
| `display_name` | plain, at most 80 characters |
| `status` | `active`, `disabled`, `pending` (invited, not yet set up) |
| `roles` | list of role ids |
| `auth` | `password` or `oidc:<provider>` |

The first user is created by `riehub init` or the setup page on an
empty instance, and gets the `owner` role. After that, users join by
invitation: an admin creates a `pending` user and a one-time invite
link valid for 72 hours. Open signup exists but is off by default
(`identity.signup = "invite"`).

Deleting a user removes the vault and every record the user owns. The
vault keys are gone with it, so backups of that user's data are
unreadable.

## 2. Password login

- Passwords: 12 to 1024 bytes. No composition rules. The 10 000 most
  common passwords are rejected from an embedded list.
- Signup: make salt and Argon2 parameters, derive verifier and slot
  key, create the vault with slot `pw`, show the recovery key once and
  require the user to confirm they saved it.
- Login: load the user by email, run Argon2id, `ct_eq` the verifier,
  unlock the vault with slot `pw`, create a session. An unknown email
  runs the same Argon2id work on a dummy record, so timing does not
  reveal which emails exist. The error is always "invalid email or
  password".
- At most 4 Argon2id computations run at once. Others wait up to 5
  seconds, then get `503`.
- Throttling: 5 failures per account per 15 minutes and 20 per IP per
  15 minutes, then `429` with `Retry-After`. Counters live in the store.
- Change password: needs the current password, re-wraps slot `pw`, and
  ends every other session.
- Recovery: the user presents the recovery key and a new password. The
  server recovers the vault key, replaces slot `pw`, and removes every
  `ws.*` slot.
- Admin reset: sets the user to `pending` with a new invite link. The
  vault stays closed. The user needs the recovery key to keep their
  data, or chooses "start empty", which deletes the old vault.

## 3. Sessions

A web session is the token `rs_<SID>_<secret>` in the cookie
`__Host-rs`. Creating one adds vault slot `ws.<SID>`. Any node can
serve the session, because the cookie alone unlocks the vault.

| Setting | Default |
|---|---|
| Idle timeout | 30 minutes |
| Absolute lifetime | 12 hours |
| Sessions per user | 10, the oldest is ended |

Ending a session (logout, timeout, password change) deletes the session
record and the slot. Users see and end their own sessions. Admins can
end all sessions of a user, which removes the session records. The
slots are cleaned by the user's next login.

## 4. Hub keys

A hub key is the user's credential for agents, SDKs and the CLI:
`rh_<ID>_<secret>`. It is shown once. Creating one adds vault slot
`hk.<ID>`.

| Field | Notes |
|---|---|
| `id` | `hk_<ULID>` |
| `scopes` | any of `ai` (router surfaces), `mcp` (MCP gateway), `manage` (management API as the user) |
| `acl` | vault path prefixes the key may read. Default: all connections and connectors of the user |
| `models` | optional allowlist of model ids and routing groups |
| `tools` | optional allowlist of MCP tools |
| `limits` | optional rate limit and budget ([governance](governance.md)) |
| `expires_at` | optional |
| `last_used_at` | updated at most once per minute |

The label of a key is content and is stored encrypted. Accepted on the
router surfaces as `Authorization: Bearer`, `x-api-key` or
`x-goog-api-key`, so each vendor SDK works unchanged. A key in a URL
query is accepted for the Gemini surface only, and query strings are
never logged.

A hub key cannot create another hub key or change roles, even with the
`manage` scope. Those actions need a web session.

## 5. Roles and permissions

Built-in roles:

| Role | Permissions |
|---|---|
| `owner` | everything below, plus instance settings and role assignment. At least one owner always exists |
| `admin` | `users:manage`, `teams:manage`, `catalog:manage`, `policy:manage`, `usage:read:all`, `audit:read` |
| `member` | their own resources, `usage:read:own` |

Custom roles (version 0.7.0) combine the same permission strings, plus
the scopes `usage:read:team`. No permission exists that grants reading
content or secrets of another user, so no role can be built that
breaks S3.

Authorization is decided in one place, the route table
([API](api.md), section 2). Handlers receive an `AuthUser` and build a
`UserScope` from it. There is no way to build a `UserScope` for someone
else.

## 6. Single sign-on (version 0.7.0)

- OIDC authorization code flow with PKCE against any compliant
  provider (Okta, Entra ID, Keycloak, Zitadel, Google Workspace).
  Client secrets are in the instance coffer.
- Login proves who the user is. It does not open the vault, because the
  identity provider is not a user-held secret. An SSO user sets a
  **vault passphrase** (slot `vp`) or a passkey (slot `pk.*`) at first
  login and presents it once per session.
- Group claims map to teams and roles by admin rules. SCIM 2.0
  provisioning creates, disables and deletes users.
- Passkeys (WebAuthn) work for password accounts too. With the PRF
  extension a passkey unlocks the vault by itself. TOTP is available as
  a second factor for password login.
