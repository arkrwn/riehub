# M00 Foundation - version 0.0.1

A workspace that builds, a gate that bites, and the four base layers:
core types, crypto, store, audit. No user-facing feature yet.

## T-0001 Toolchain and gate
- Version: 0.0.1
- Depends: none
- Review: standard
- Type: chore
- Spec: docs/adr/0003-software-factory.md, factory/README.md
- Paths: deny.toml, Makefile, scripts/, .github/workflows/ci.yml, rust-toolchain.toml

### Build
1. Run `make tools`. It installs `cargo-deny`.
2. Create `deny.toml` with `cargo deny init`, then set: advisories deny
   on vulnerabilities and unmaintained crates, licenses allowlist
   (`Apache-2.0`, `MIT`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`,
   `Unicode-3.0`, `Zlib`, `MPL-2.0`, `OpenSSL`, `CC0-1.0`), bans deny
   on `openssl-sys` and `native-tls`, sources allow crates.io and the
   git URL `https://github.com/arkrwn/cofferak` only.
3. Make `.github/workflows/ci.yml` run `make gate` on `ubuntu-latest`
   and `macos-latest`. Install `cargo-deny` in CI with `make tools`.
   Cache `~/.cargo` and `target`.
4. Confirm `rust-toolchain.toml` pins the exact version and CI uses it.

### Accept
- `make gate` exits 0 on a clean checkout.
- `make security` exits non-zero when `deny.toml` is temporarily
  renamed (prove it once, then restore).
- `scripts/rules-lint.sh` exits non-zero when a file `crates/x/src/lib.rs`
  containing `println!("x");` is added (prove it once, then remove).

### Proof
`make gate`

## T-0002 Cargo workspace and crate skeletons
- Version: 0.0.1
- Depends: T-0001
- Review: standard
- Type: chore
- Spec: docs/SPEC.md section 3.1, docs/adr/0002-rust-single-binary-linux-macos.md
- Paths: Cargo.toml, src/ (remove), crates/

### Build
1. Turn the root `Cargo.toml` into a virtual workspace: `resolver = "2"`,
   `members = ["crates/*"]`, `[workspace.package]` with `version =
   "0.0.1"`, `edition = "2021"`, `license = "Apache-2.0"`, `repository`,
   `rust-version` equal to the pinned toolchain. Keep the line
   `version = "0.0.1"` at column 0 (the release script greps it).
2. Add `[workspace.lints]`: rust `unsafe_code = "forbid"`,
   `missing_debug_implementations = "warn"`. clippy `unwrap_used`,
   `expect_used`, `panic`, `dbg_macro`, `print_stdout`, `print_stderr`
   = "deny", with `allow` only inside `#[cfg(test)]`.
3. Create library crates `riehub-core`, `riehub-crypto`,
   `riehub-store`, `riehub-audit` and the binary crate `riehub-server`
   (binary name `riehub`). Each: `lints.workspace = true`,
   `#![forbid(unsafe_code)]` at the crate root, one smoke test.
4. `riehub version` prints `riehub <version>`. Move the seed test
   there. Delete the root `src/`.
5. Dependencies point downward only, as in SPEC section 3.1.

### Accept
- `cargo build --workspace` and `make check` pass.
- Test `version_prints_workspace_version` runs the binary and checks
  the output starts with `riehub 0.0.1`.
- `scripts/rules-lint.sh` passes, including the forbid check for each
  crate root.

### Proof
`cargo test --workspace`

## T-0003 Core types: ids, Secret, Content, errors, clock
- Version: 0.0.1
- Depends: T-0002
- Review: security
- Spec: docs/spec/security.md section 2, docs/spec/storage.md section 2
- Paths: crates/riehub-core/

### Build
1. Typed ids: a macro that defines `UserId`, `SessionId`, `HubKeyId`,
   `ConnectionId`, `RequestId` and later ones as `<prefix>_<ULID>`.
   Parsing rejects a wrong prefix. Use the `ulid` crate.
2. `Secret<T: Zeroize>`: zeroizes on drop. `Debug` and `Display` print
   `[secret]`. No `Serialize`, no `Clone`, no `PartialEq`. Access with
   `.expose()`.
3. `Content`: owned bytes, same rules, prints `[content]`. It
   implements `Deserialize` (it is parsed from requests) and offers
   `expose()` and `len()`.
4. `Error` enum with the classes `NotFound`, `Conflict`, `Unauthorized`,
   `Forbidden`, `Invalid`, `RateLimited`, `Unavailable`, `Internal`.
   `Display` never includes a source error's text for `Internal`.
5. `Clock` trait with `SystemClock` and a `TestClock`.
6. `Owner` enum: `Instance`, `User(UserId)`.

### Accept
- `secret_debug_and_display_are_redacted`: formatting a
  `Secret<String>` holding `hunter2` with `{:?}` and `{}` never
  contains `hunter2`.
- `content_debug_is_redacted`, same for `Content`.
- A `trybuild` compile-fail test proves `serde_json::to_string(&secret)`
  does not compile.
- `id_rejects_wrong_prefix`, `id_round_trips`.
- `internal_error_display_hides_source`.

### Proof
`cargo test -p riehub-core`

## T-0004 Configuration loading
- Version: 0.0.1
- Depends: T-0003
- Review: security
- Spec: docs/spec/api.md section 3
- Paths: crates/riehub-core/src/config.rs, crates/riehub-server/

### Build
1. `Config` struct exactly as the TOML in the spec, with `serde` and
   `deny_unknown_fields`. Use the `toml` crate.
2. Load order: defaults, file, then `RIEHUB_*` environment variables
   with `__` as the level separator.
3. `Config::validate(profile)` implements the profile rules in the
   spec. Errors name the key, never a value.
4. `riehub serve --config <path>` loads and validates, `riehub doctor`
   prints each check as `ok` or `fail`.

### Accept
- `defaults_are_dev_loopback_8080`.
- `env_overrides_file`.
- `unknown_key_is_rejected`.
- `production_without_tls_or_trusted_proxy_is_rejected`.
- `production_with_http_public_url_is_rejected`.
- `dev_on_non_loopback_is_rejected`.
- `validation_error_does_not_echo_values`.

### Proof
`cargo test -p riehub-core config`

## T-0005 Crypto primitives
- Version: 0.0.1
- Depends: T-0003
- Review: security
- Spec: docs/spec/security.md section 1
- Paths: crates/riehub-crypto/

### Build
1. Implement exactly the eight functions in the spec, on `aws-lc-rs`
   and the `argon2` crate. `Key32` as specified.
2. `aead_seal` output is `0x01 || nonce(12) || ciphertext+tag`.
   `aead_open` rejects any other first byte and any short input.
3. `Argon2Params { m_kib, t, p }` with `DEFAULT` = 65536, 3, 1 and a
   `TEST` value (8 MiB, 1, 1) used only under `#[cfg(test)]` and a
   `test-util` feature.
4. `hkdf` panics at compile time, not run time: take `info` as
   `&'static str` and assert in a test that every call site string
   starts with `riehub/v1/`.

### Accept
- Known-answer tests: HKDF-SHA384 (RFC 5869 style vector computed
  with `aws-lc-rs` directly), HMAC-SHA384 (RFC 4231 case 1), SHA-384
  of `abc`, Argon2id against the `argon2` crate's own reference vector.
- `aead_round_trips`.
- `aead_rejects_wrong_key`, `aead_rejects_wrong_aad`,
  `aead_rejects_flipped_bit`, `aead_rejects_truncated`,
  `aead_rejects_unknown_version_byte`.
- `two_seals_of_same_input_differ` (random nonce).
- `ct_eq_matches_eq_for_random_inputs` (property test, `proptest`).
- `key32_has_no_debug_output` (compile-fail with `trybuild`).

### Proof
`cargo test -p riehub-crypto`

## T-0006 Store trait and SQLite backend
- Version: 0.0.1
- Depends: T-0003
- Review: security
- Spec: docs/spec/storage.md, docs/adr/0006-storage-trait-and-backends.md
- Paths: crates/riehub-store/

### Build
1. Define `Record`, `Kind`, `Expect`, `Filter`, `Page`, `Listed` and the
   `Store` trait as in the spec. Leave `audit_*` and `usage_*` as trait
   methods with the SQLite implementation included.
2. SQLite backend on `sqlx` (features `sqlite`, `runtime-tokio`,
   `tls-rustls` off, bundled SQLite). Open with WAL, `synchronous=FULL`,
   `foreign_keys=ON`, `busy_timeout=5000`. Create the file with mode
   0600 and refuse a file that is group or world readable.
3. Migrations in `migrations/sqlite/0001_init.sql`: tables `records`
   (primary key owner, kind, id. unique owner, kind, lookup),
   `audit`, `counters`, `usage`, `usage_buckets`, `leases`,
   `schema_version`. All queries are parameterised.
4. `conformance` module, public behind the `conformance` feature: a
   function `run(store_factory)` with one test function per rule of
   spec section 3.
5. `open(url)` picks the backend by scheme. Unknown schemes are an
   error.

### Accept
- The conformance suite passes on SQLite. It includes at least:
  `put_new_then_get`, `put_new_twice_conflicts`,
  `put_wrong_version_conflicts`, `lookup_is_unique_per_owner_and_kind`,
  `get_with_other_owner_is_none`, `list_never_crosses_owners`,
  `list_filters_parent_status_sort`, `list_pages_with_cursor`,
  `delete_owner_removes_everything_of_that_owner_only`,
  `counter_add_is_atomic_under_concurrency` (100 tasks),
  `lease_is_exclusive_until_expiry`,
  `audit_append_is_gapless_under_concurrency`,
  `usage_query_groups_and_filters`.
- `db_file_is_0600`, `refuses_world_readable_db`.
- `migrate_is_idempotent`, `newer_schema_is_refused`.

### Proof
`cargo test -p riehub-store`

## T-0007 Audit log
- Version: 0.0.1
- Depends: T-0005, T-0006
- Review: security
- Spec: docs/spec/security.md section 6, docs/adr/0010-audit-log.md
- Paths: crates/riehub-audit/, crates/riehub-server/src/cli/

### Build
1. `AuditEvent` enum with the variants needed so far: `SystemStart`,
   `ConfigLoad`, `AuditPrune`. Later tasks add variants. Each variant's
   fields are ids, enums, numbers, booleans.
2. Canonical bytes: field order fixed by a hand-written encoder
   (length-prefixed fields), not by JSON key order.
3. `AuditLog::append(events)` computes the MAC chain through
   `Store::audit_append`. A writer task batches for up to 10 ms.
   `append` returns only after the batch is durable.
4. `AuditLog::verify(from, to)` returns the first bad `seq` or `Ok`.
5. CLI: `riehub audit verify` (exit 3 on a break), `audit head`,
   `audit export` (JSON lines).
6. The audit key comes from a `KeySource` trait. Until T-0102 the
   server uses a key file in the data directory, mode 0600, created at
   first start.

### Accept
- `chain_verifies_after_1000_appends`.
- `verify_detects_modified_event`, `verify_detects_deleted_event`,
  `verify_detects_reordered_events`, `verify_detects_wrong_key`.
- `append_fails_when_store_fails` (no silent drop).
- `event_enum_has_no_string_fields`: a test that serialises one sample
  of every variant and asserts every JSON string value parses as a
  typed id or a known enum value.
- `cli_verify_exits_3_on_tamper`.

### Proof
`cargo test -p riehub-audit`

## T-0008 HTTP server skeleton
- Version: 0.0.1
- Depends: T-0004, T-0007
- Review: security
- Spec: docs/spec/api.md sections 1-2, docs/spec/security.md section 7
- Paths: crates/riehub-server/

### Build
1. `axum` application built only from the static route table
   `routes.rs` (`RouteSpec`, classes as in the spec). For now the table
   holds `GET /healthz` and `GET /readyz`, class `Public`.
2. Middleware, in order: request id, body and header limits, timeout,
   security headers, structured access log with the allowlisted fields
   `request_id, method, route_template, status, duration_ms`. Never
   the path with ids, the query string or any header value.
3. Problem JSON error mapping from `riehub_core::Error`.
4. Process hardening at start with the `nix` crate: core limit 0,
   non-dumpable on Linux.
5. `riehub init` creates the data directory (mode 0700) and runs
   migrations. `riehub serve` binds per config, writes `SystemStart` to
   the audit log, and shuts down gracefully on `SIGTERM` and `SIGINT`.
6. `tracing` with JSON output on stdout.

### Accept
- `healthz_is_200`, `readyz_is_503_when_store_is_down`.
- `responses_carry_security_headers` (each header in the spec).
- `oversized_body_is_413`, `slow_request_times_out`.
- `access_log_has_only_allowlisted_fields`: capture the log, parse
  each line as JSON, assert the key set.
- `unknown_route_is_404_problem_json`.
- `every_route_has_a_class` (iterates the table).
- `serve_writes_system_start_audit_event`.
- `graceful_shutdown_finishes_inflight_request`.

### Proof
`cargo test -p riehub-server`

## T-0009 Privacy canary harness
- Version: 0.0.1
- Depends: T-0008
- Review: security
- Type: test
- Spec: docs/spec/security.md section 8
- Paths: crates/riehub-server/tests/

### Build
1. `tests/common/mod.rs`: `TestServer::start()` boots the real binary
   on a temp data directory and a free loopback port with log level
   `trace`, captures stdout and stderr, and stops it on drop.
2. `scan_for(canary) -> Vec<Hit>` searches every file under the data
   directory and the captured output, as raw bytes and as base64 and
   hex encodings of the canary.
3. `tests/privacy_canary.rs`: sends the canary in the body, a header,
   the query string and the path of requests to the routes that exist,
   then asserts no hit.

### Accept
- `canary_never_reaches_disk_or_logs` passes.
- `scanner_finds_a_planted_canary`: write the canary to a file in the
  data directory and assert the scanner reports it (proves the scanner
  can fail).

### Proof
`cargo test -p riehub-server --test privacy_canary`

## T-0099 Release 0.0.1
- Version: 0.0.1
- Depends: T-0001, T-0002, T-0003, T-0004, T-0005, T-0006, T-0007, T-0008, T-0009
- Review: standard
- Type: chore
- Spec: SDD.md section "Releases"
- Paths: CHANGELOG.md, README.md, docs/getting-started.md, docs/development.md

### Build
1. In `CHANGELOG.md` move `Unreleased` into `## [0.0.1] - <today>` and
   leave an empty `Unreleased`. Fix the compare links.
2. Update the version badge in `README.md`. Update
   `docs/getting-started.md` so it describes exactly what works now:
   build, `riehub init`, `riehub serve`, `curl /healthz`,
   `riehub audit verify`.
3. Update the source layout in `docs/development.md`.
4. After `finish`, run `factory/factory.sh release 0.0.1`.

### Accept
- Every command in `docs/getting-started.md` was run and its output
  matches the page.
- `factory/factory.sh release 0.0.1` prints `RELEASED v0.0.1`.

### Proof
`make gate`
