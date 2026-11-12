# M01 Identity and vault - version 0.1.0

Users can sign up, log in, hold a vault only they can open, create hub
keys, and see it all in the first screens of the web UI.

## T-0101 Per-user vault on cofferak-core
- Version: 0.1.0
- Depends: T-0005, T-0006, T-0007
- Review: security
- Spec: docs/spec/security.md section 3, docs/adr/0005-user-vault-on-cofferak.md
- Paths: Cargo.toml, deny.toml, crates/riehub-vault/

### Build
1. Add the workspace dependency `cofferak-core = { git =
   "https://github.com/arkrwn/cofferak", rev =
   "ad45409e113838e67ea9b80052dd2330dc7aa101" }`.
2. New crate `riehub-vault`. Implement the operations listed in the
   spec, section 3.3, on `cofferak_core::format::VaultFile` and
   `cofferak_core::crypto`. `Unlocked` holds the vault key in a
   zeroizing buffer and has no `Debug`.
3. Slot ids are validated with `cofferak_core::acl::validate_id`.
4. `save` stores `VaultFile::to_bytes()` as the record kind `vault`
   (one per user, id = user id) with `Expect::Version`, and appends the
   audit event `VaultWrite { user, generation }` (add the variant).
5. `unlock` verifies the MAC with `VaultFile::verify`, and refuses a
   generation lower than the newest `VaultWrite` for that user.
6. `content_key(kind)` is `hkdf(dek, "riehub/v1/content/<kind>")` with
   a fixed enum of kinds, each mapped to a `&'static str`.
7. `seal_record` and `open_record` helpers implement the record AAD of
   spec section 4.

### Accept
- `create_then_unlock_with_slot`, `unlock_with_wrong_key_fails`.
- `recover_with_recovery_key`, `recover_with_other_users_key_fails`.
- `add_and_remove_slot`, `removed_slot_no_longer_unlocks`.
- `set_get_remove_entry`, `entry_over_1mib_is_rejected`.
- `get_outside_acl_is_denied` (prefix `conn/con_A` cannot read
  `conn/con_B/credential`).
- `tampered_vault_bytes_fail_to_unlock`.
- `rolled_back_vault_is_refused`.
- `concurrent_saves_one_wins_one_conflicts`.
- `record_blob_fails_to_open_for_other_user_or_id_or_version`.
- `stored_vault_bytes_do_not_contain_the_entry_plaintext`.

### Proof
`cargo test -p riehub-vault`

## T-0102 Instance coffer and host seals
- Version: 0.1.0
- Depends: T-0101
- Review: security
- Spec: docs/spec/security.md section 5
- Paths: crates/riehub-vault/src/instance.rs, crates/riehub-server/

### Build
1. `InstanceCoffer`: a `VaultFile` stored as an instance record, with
   one slot `instance` whose key is the host key.
2. Host key backends behind `instance.seal`: `systemd`
   (`SystemdCredsSeal`), `keychain` (`KeychainSeal`, macOS only),
   `file` (32 bytes, mode 0600, refuse other modes). Tests use `file`.
3. `riehub init` creates the coffer, stores the audit key and the
   cookie key in it, and prints the armored instance recovery key once
   to the terminal (this is the one allowed print, mark it for the
   rules lint).
4. Replace the temporary audit key file from T-0007 with the coffer
   entry `audit/key`. `riehub doctor` checks the seal can be opened.

### Accept
- `init_creates_coffer_and_prints_recovery_key_once`.
- `serve_fails_closed_when_host_key_is_missing_or_wrong`.
- `key_file_with_loose_mode_is_refused`.
- `audit_chain_verifies_with_key_from_coffer`.
- `instance_recovery_key_reopens_the_coffer`.
- The canary test still passes.

### Proof
`cargo test -p riehub-vault instance && cargo test -p riehub-server`

## T-0103 Users, setup and password login
- Version: 0.1.0
- Depends: T-0102
- Review: security
- Spec: docs/spec/identity.md sections 1-2
- Paths: crates/riehub-identity/, crates/riehub-server/

### Build
1. New crate `riehub-identity`. `User` record (instance-owned, lookup =
   lower-cased email) and a `credential` record per user: salt, Argon2
   parameters, verifier.
2. `POST /api/v1/setup` (works only while no user exists) creates the
   owner. `POST /api/v1/auth/login`. Both `Public`.
3. Signup creates the vault with slot `pw` and returns the armored
   recovery key once in the response body, typed as `Secret` and
   written through a response type that is the only place allowed to
   expose it.
4. Login as in the spec: dummy work for unknown emails, one error
   message, a semaphore of 4 Argon2 computations, throttling counters
   in the store.
5. Embedded common-password list (10 000 entries, one per line, in
   `crates/riehub-identity/data/`).
6. Audit variants: `UserCreate`, `LoginOk`, `LoginFailed`,
   `LoginThrottled`.

### Accept
- `setup_creates_owner_once`, `second_setup_is_409`.
- `login_ok_with_correct_password`.
- `login_unknown_email_and_wrong_password_give_same_status_and_body`.
- `login_is_throttled_after_5_failures` returns 429 with `Retry-After`.
- `common_password_is_rejected`, `short_password_is_rejected`.
- `argon2_concurrency_is_capped` (5th waits).
- `stored_credential_never_contains_password`: scan the database file.
- The canary test covers the password and the recovery key.

### Proof
`cargo test -p riehub-identity && cargo test -p riehub-server`

## T-0104 Web sessions
- Version: 0.1.0
- Depends: T-0103
- Review: security
- Spec: docs/spec/identity.md section 3, docs/spec/security.md sections 3.2, 3.4, 7
- Paths: crates/riehub-identity/, crates/riehub-server/

### Build
1. On login create token `rs_<SID>_<secret>`, add slot `ws.<SID>`,
   store the session record (verifier, user, created, last seen,
   expiry) and set cookie `__Host-rs`.
2. `AuthUser` extractor: parse cookie, load session, `ct_eq` verifier,
   check idle and absolute timeouts, unlock the vault through the
   unlock cache, update last seen at most once per minute.
3. CSRF: require `X-CSRF-Token` on every non-GET `/api` request made
   with a cookie. `GET /api/v1/me` returns the token.
4. `POST /api/v1/auth/logout`, `GET /api/v1/me/sessions`,
   `DELETE /api/v1/me/sessions/{id}`. Ending a session deletes the
   record and the slot.
5. The unlock cache as specified (10 minutes idle, 10 000 entries).
6. In profile `dev` over plain HTTP the cookie is named `rs` without
   the `__Host-` prefix and without `Secure`. Nowhere else.

### Accept
- `login_sets_host_cookie_with_flags`.
- `session_reaches_me_and_unlocks_vault`.
- `idle_timeout_ends_session`, `absolute_timeout_ends_session`
  (TestClock).
- `logout_removes_session_and_slot`.
- `post_without_csrf_token_is_403`.
- `eleventh_session_ends_the_oldest`.
- `session_works_on_a_second_server_instance_sharing_the_store` (two
  `TestServer`s on one database file, started one after the other).
- `stolen_session_record_from_db_cannot_unlock`: build a token from
  the stored verifier and fail.
- `unlock_cache_evicts_after_idle`.

### Proof
`cargo test -p riehub-identity session && cargo test -p riehub-server`

## T-0105 Roles, route classes and user administration
- Version: 0.1.0
- Depends: T-0104
- Review: security
- Spec: docs/spec/identity.md section 5, docs/spec/api.md section 2, docs/SPEC.md section 2.1
- Paths: crates/riehub-identity/, crates/riehub-server/

### Build
1. Roles `owner`, `admin`, `member` and the permission enum. The
   constructor of `UserScope` is private to `riehub-identity` and takes
   an `AuthUser`.
2. Enforce route classes in one middleware driven by the route table.
   Marker trait `AdminSafe` for response types of `AdminMeta` routes,
   implemented by hand only for types without `Content` or `Secret`.
3. Admin routes: `GET/POST /api/v1/admin/users`, `PATCH
   /api/v1/admin/users/{id}` (roles, status), `DELETE` (removes every
   record of the user with `delete_owner`), `POST
   /api/v1/admin/invites`, `GET /api/v1/admin/audit`.
4. Public `GET/POST /api/v1/invites/{token}`: accept an invite, set a
   password, create the vault.
5. Generated class tests that iterate the route table, as in the spec.
6. The last owner cannot be deleted, disabled or demoted.

### Accept
- `class_tests_cover_every_route` (the generated suite, and a test that
  fails if a route is missing from it).
- `member_gets_403_on_admin_routes`.
- `owner_gets_404_for_another_users_session_id`.
- `admin_user_list_has_no_secret_or_content_fields`.
- `delete_user_removes_vault_and_all_records`.
- `invite_expires_after_72h`, `invite_is_single_use`.
- `last_owner_cannot_be_removed`.
- `admin_cannot_create_session_for_another_user`: no route accepts a
  target user id for login, and the test asserts the table has none.

### Proof
`cargo test -p riehub-server`

## T-0106 Hub keys
- Version: 0.1.0
- Depends: T-0105
- Review: security
- Spec: docs/spec/identity.md section 4, docs/spec/security.md sections 3.2-3.4
- Paths: crates/riehub-identity/, crates/riehub-server/

### Build
1. `POST /api/v1/me/keys` (session only) creates `rh_<ID>_<secret>`,
   adds slot `hk.<ID>`, stores verifier, scopes, ACL prefixes, expiry.
   The label is an encrypted record field. The token is returned once.
2. `GET /api/v1/me/keys`, `PATCH`, `DELETE` (removes the slot).
3. `KeyAuth` extractor for classes `Key(scope)`: accepts
   `Authorization: Bearer`, `x-api-key`, `x-goog-api-key`. Checks
   verifier, expiry, scope, then unlocks through the cache.
4. A `manage` key may call `Self_` routes except creating keys and
   changing password.
5. A test-only route class `Key(Ai)` endpoint `GET /v1/models` that
   returns an empty list, so the extractor is testable now.

### Accept
- `key_is_shown_once_and_not_retrievable`.
- `key_auth_works_with_each_of_the_three_headers`.
- `revoked_key_is_401_within_one_request`.
- `expired_key_is_401`, `key_without_scope_is_403`.
- `key_cannot_create_keys`.
- `key_unlocks_vault_without_any_session`.
- `key_acl_limits_vault_reads`.
- `db_contents_cannot_rebuild_a_key`.

### Proof
`cargo test -p riehub-identity key && cargo test -p riehub-server`

## T-0107 Password change, recovery and admin reset
- Version: 0.1.0
- Depends: T-0106
- Review: security
- Spec: docs/spec/identity.md section 2
- Paths: crates/riehub-identity/, crates/riehub-server/

### Build
1. `POST /api/v1/me/password`: current and new password. Re-wrap slot
   `pw`, new salt and verifier, end other sessions.
2. `POST /api/v1/auth/recover`: email, recovery key, new password.
3. `POST /api/v1/admin/users/{id}/reset`: status `pending`, new
   invite. Accepting it offers "I have my recovery key" or "start
   empty".
4. `POST /api/v1/me/recovery-key/rotate`: new root keypair, new
   `root_wrap`, show the new key once.

### Accept
- `change_password_keeps_vault_entries_readable`.
- `old_password_stops_working`, `other_sessions_end`.
- `recover_restores_access_and_drops_all_sessions`.
- `recover_with_wrong_key_fails_without_side_effects`.
- `admin_reset_does_not_open_vault`: after reset and "start empty",
  the old entries are gone, and before it nothing can read them.
- `rotated_recovery_key_works_and_old_one_fails`.

### Proof
`cargo test -p riehub-server recover`

## T-0108 Web app shell and design system
- Version: 0.1.0
- Depends: T-0105
- Review: security
- Spec: docs/spec/web.md, docs/adr/0011-web-ui-stack.md, docs/spec/security.md section 7
- Paths: web/, crates/riehub-server/, Makefile, scripts/, .github/workflows/ci.yml, docs/api/

### Build
1. Scaffold `web/` with the stack in the spec. Commit
   `package-lock.json`. Pin exact versions.
2. `tokens.css` with every token of spec section 2, light and dark.
   Tailwind theme reads the variables. Self-host Inter.
3. Build the components listed in spec section 4 that the pages of
   this milestone need: `Card`, `Button`, `IconButton`, `Pill`,
   `NavPills`, `Rail`, `Input`, `Dialog`, `Toast`, `SecretReveal`,
   `EmptyState`, `ErrorState`. One Vitest test each.
4. The app shell of spec section 3: top bar, rail, theme toggle,
   responsive rule.
5. Pages: Setup, Login, Recover, Accept invite. They call the real API.
6. `riehub openapi` (use `utoipa`) writes `docs/api/openapi.json`.
   Generate `web/src/api/types.ts` from it.
7. Embed `web/dist` with `rust-embed`. Serve it with the CSP from the
   spec, long cache for hashed assets, `no-store` for `index.html`.
8. Makefile: `web-build`, `web-test`, `web-lint`. `build` depends on
   `web-build`. `check` runs `web-lint`, `web-test` and a drift check
   that `docs/api/openapi.json` and `types.ts` are current. Add Node
   to `scripts/install-tools.sh` checks and to CI.

### Accept
- `make check` runs the web lint, unit tests and drift check.
- `index_html_is_served_with_csp_and_no_store`.
- `built_app_has_no_inline_script_or_style`: a test scans `web/dist`.
- `rules-lint` third-party origin rule passes.
- Vitest: each component renders, is keyboard operable, and
  `SecretReveal` clears its value on close.
- `npm audit --audit-level=high` is clean.
- A screenshot of the Login page in both themes is attached to
  `factory/reviews/T-0108.md` as file paths under `web/e2e/__shots__`.

### Proof
`make web-test && cargo test -p riehub-server static`

## T-0109 Web: account pages and end-to-end smoke
- Version: 0.1.0
- Depends: T-0107, T-0108
- Review: standard
- Spec: docs/spec/web.md section 5, docs/spec/identity.md
- Paths: web/, Makefile, .github/workflows/ci.yml

### Build
1. Pages: Profile, Security (sessions, hub keys, change password,
   rotate recovery key), Appearance, and Admin Users with invites and
   the audit viewer.
2. Add `DataTable`, `StatusDot`, `Select`, `Toggle`.
3. Playwright suite `web/e2e` against a real `riehub` started on a
   temp directory: setup, save recovery key, logout, login, create and
   revoke a hub key, invite a second user, accept, verify the second
   user cannot see the first user's keys, recover with the recovery
   key.
4. Makefile target `e2e`. Add it to `gate` after `check`.

### Accept
- `make e2e` passes headless on Linux.
- The journey in step 3 is one test per step, in both themes for the
  visual steps.
- An automated accessibility check (`@axe-core/playwright`) reports no
  serious or critical issue on each page.

### Proof
`make e2e`

## T-0199 Release 0.1.0
- Version: 0.1.0
- Depends: T-0101, T-0102, T-0103, T-0104, T-0105, T-0106, T-0107, T-0108, T-0109
- Review: standard
- Type: chore
- Spec: SDD.md section "Releases"
- Paths: Cargo.toml, web/package.json, CHANGELOG.md, README.md, docs/

### Build
1. Set `version = "0.1.0"` in the root `Cargo.toml` and in
   `web/package.json`. Run `cargo build` so `Cargo.lock` follows.
2. Move `Unreleased` to `## [0.1.0] - <today>` in `CHANGELOG.md`.
3. Update the README badge and Quickstart, `docs/getting-started.md`
   and `docs/overview.md` to what works now. Add
   `scripts/quickstart-check.sh` that runs the README Quickstart
   commands, and call it from `make e2e`.
4. After `finish`, run `factory/factory.sh release 0.1.0`.

### Accept
- `scripts/quickstart-check.sh` exits 0.
- `factory/factory.sh release 0.1.0` prints `RELEASED v0.1.0`.

### Proof
`make gate`
