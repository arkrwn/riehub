# M07 Enterprise identity and observability - version 0.7.0

Single sign-on, stronger login, finer roles, and the signals operators
need, all without content.

## T-0701 OIDC single sign-on and the vault passphrase
- Version: 0.7.0
- Depends: T-0606
- Review: security
- Spec: docs/spec/identity.md section 6
- Paths: crates/riehub-identity/src/oidc.rs, crates/riehub-server/

### Build
1. Admin routes `admin/sso`: issuer, client id, client secret (to the
   instance coffer), scopes, claim mapping rules for teams and roles.
2. Login: authorization code with PKCE and nonce, discovery, ID token
   validation (issuer, audience, expiry, nonce, signature against the
   JWKS with key rotation), using `riehub-oauth` and a maintained JWT
   crate on `aws-lc-rs`.
3. First login creates the user (`auth = oidc:<provider>`) and requires
   a vault passphrase: slot `vp`, the recovery key is shown once.
4. Later logins authenticate with the provider, then ask for the
   passphrase once per session to add the `ws.<SID>` slot. Until then
   the session is "locked": it can reach only the unlock route.
5. Group claims map to teams and roles on every login.

### Accept
- Against a mock OIDC provider: `first_login_creates_user_and_vault`.
- `login_without_passphrase_cannot_reach_self_routes`.
- `wrong_nonce_or_audience_or_issuer_or_signature_is_refused` (four
  cases).
- `expired_id_token_is_refused`.
- `jwks_rotation_is_followed`.
- `group_claim_maps_to_team_and_role`.
- `idp_cannot_unlock_vault`: a valid ID token for the user without
  the passphrase reads nothing.

### Proof
`cargo test -p riehub-identity oidc && cargo test -p riehub-server sso`

## T-0702 Passkeys and TOTP
- Version: 0.7.0
- Depends: T-0701
- Review: security
- Spec: docs/spec/identity.md section 6, docs/spec/security.md sections 3.1-3.2
- Paths: crates/riehub-identity/, crates/riehub-server/, web/

### Build
1. WebAuthn registration and authentication with the `webauthn-rs`
   crate. Relying party id from `server.public_url`.
2. PRF extension: when the authenticator supports it, the PRF output
   derives slot `pk.<ID>`, so the passkey both logs in and unlocks.
   Without PRF the passkey only logs in and the password or passphrase
   is still needed to unlock.
3. TOTP (RFC 6238) as a second factor for password login. The user's
   vault is still locked when the code is checked, so the seed is kept
   in the instance coffer at `totp/<user_id>`. Ten single-use recovery
   codes are stored as verifiers. TOTP protects login only. It adds
   nothing to vault encryption, and the docs say so.
4. Web: Security page sections for passkeys and TOTP.

### Accept
- `passkey_registration_and_login` with a software authenticator.
- `passkey_with_prf_unlocks_vault`, `passkey_without_prf_does_not`.
- `totp_required_after_password_when_enabled`.
- `totp_code_cannot_be_reused_within_its_window`.
- `recovery_code_is_single_use`.
- `removing_a_passkey_removes_its_slot`.

### Proof
`cargo test -p riehub-identity mfa` (put the tests of this card in a module named `mfa`)

## T-0703 SCIM provisioning
- Version: 0.7.0
- Depends: T-0701
- Review: security
- Spec: docs/spec/identity.md section 6
- Paths: crates/riehub-identity/src/scim.rs, crates/riehub-server/

### Build
1. SCIM 2.0 under `/api/v1/scim/v2`: `Users` and `Groups` with create,
   get, list with filter on `userName` and `externalId`, replace,
   patch, delete. `ServiceProviderConfig`, `Schemas`, `ResourceTypes`.
2. Auth: a SCIM token created by an owner, stored as a verifier.
3. Deprovisioning disables first. Delete removes the user and the
   vault.
4. Groups map to teams.

### Accept
- `scim_create_patch_deactivate_delete_user`.
- `scim_group_membership_maps_to_team`.
- `scim_token_cannot_reach_any_other_route`.
- `scim_responses_have_no_secret_or_content`.
- `scim_delete_removes_vault`.

### Proof
`cargo test -p riehub-server scim`

## T-0704 Custom roles and data access scopes
- Version: 0.7.0
- Depends: T-0701
- Review: security
- Spec: docs/spec/identity.md section 5, docs/SPEC.md section 2.1
- Paths: crates/riehub-identity/, crates/riehub-server/, web/

### Build
1. Custom roles: a name and a set of permissions from the fixed enum.
   Routes `admin/roles`.
2. Usage scopes `usage:read:own`, `usage:read:team`, `usage:read:all`
   applied in `usage_query`. Team managers get `usage:read:team` for
   their teams.
3. The permission enum is the whole list. A test asserts no permission
   name contains `content`, `secret`, `vault`, `impersonate`.
4. Web: Admin Roles page.

### Accept
- `custom_role_grants_only_its_permissions`.
- `team_manager_sees_team_usage_not_other_teams`.
- `no_permission_can_reach_a_self_route_of_another_user` (generated
  over every permission and every `Self_` route).
- `permission_enum_has_no_content_permission`.
- `role_change_takes_effect_on_next_request`.

### Proof
`cargo test -p riehub-server roles`

## T-0705 Metrics and tracing
- Version: 0.7.0
- Depends: T-0606
- Review: security
- Spec: docs/spec/deploy.md section 7
- Paths: crates/riehub-server/src/observability/

### Build
1. Prometheus text endpoint on `server.metrics_bind` only, never on
   the main listener. Metrics: request counts and latency histograms
   by route template, surface, provider kind, status. Token and cost
   counters by provider kind and catalog model. Breaker states. Store
   and audit latencies. Process metrics.
2. Label values come from enums and catalog ids only. A typed label
   API makes any other value a compile error.
3. OTLP trace export over HTTP through the guarded client to an
   admin-configured endpoint (allowlisted). Span attributes use the
   same allowlist as the access log.
4. `x-request-id` is the trace correlation id.

### Accept
- `metrics_endpoint_is_not_on_main_listener`.
- `metric_labels_have_bounded_cardinality` (no user id, no key id, no
  connection id, no free text. Assert the label set of every metric).
- `traces_have_only_allowlisted_attributes`.
- The canary test scans the metrics output and an OTLP capture.

### Proof
`cargo test -p riehub-server observability`

## T-0706 Exports and retention
- Version: 0.7.0
- Depends: T-0705
- Review: security
- Spec: docs/spec/security.md section 6, docs/spec/api.md section 2
- Paths: crates/riehub-server/src/export/, crates/riehub-audit/

### Build
1. Scheduled export of audit events and usage rows as JSON lines to a
   local directory or an S3-compatible bucket (SigV4, through the
   guarded client). Credentials in the instance coffer.
2. Each export file ends with the chain head it covers.
3. Retention: `audit.retention_days` and a usage retention setting.
   Pruning writes `AuditPrune` and keeps the chain verifiable from the
   prune point.
4. Routes `admin/exports`.

### Accept
- `export_contains_only_metadata` (canary).
- `export_file_carries_chain_head`.
- `chain_verifies_after_prune`.
- `prune_never_removes_events_younger_than_retention`.
- `export_bucket_on_private_address_is_refused_unless_allowlisted`.

### Proof
`cargo test -p riehub-server export`

## T-0707 Projects and customers
- Version: 0.7.0
- Depends: T-0704
- Review: standard
- Spec: docs/spec/governance.md section 1
- Paths: crates/riehub-governance/, crates/riehub-router/, crates/riehub-server/, web/

### Build
1. Project records (user-owned or team-owned): slug, limits. Header
   `x-riehub-project`. Unknown slugs are refused.
2. Customer records (team-owned): groups hub keys.
3. Both become levels in limits and dimensions in usage.
4. Web: Usage filters by project and customer.

### Accept
- `project_header_scopes_budget_and_usage`.
- `unknown_project_is_refused`.
- `customer_budget_applies_to_its_keys`.

### Proof
`cargo test -p riehub-server project`

## T-0799 Release 0.7.0
- Version: 0.7.0
- Depends: T-0701, T-0702, T-0703, T-0704, T-0705, T-0706, T-0707
- Review: standard
- Type: chore
- Spec: SDD.md section "Releases"
- Paths: Cargo.toml, web/package.json, CHANGELOG.md, README.md, docs/

### Build
1. Version `0.7.0` in `Cargo.toml` and `web/package.json`.
2. `CHANGELOG.md` section `## [0.7.0] - <today>`.
3. New docs pages `docs/guides/sso.md` and `docs/guides/operations.md`
   (metrics, tracing, exports), added to the nav.
4. After `finish`, run `factory/factory.sh release 0.7.0`.

### Accept
- `factory/factory.sh release 0.7.0` prints `RELEASED v0.7.0`.

### Proof
`make gate`
