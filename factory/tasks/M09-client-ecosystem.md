# M09 Client and ecosystem - version 0.9.0

The `rie` CLI that wires local coding agents to the hub, imports,
plugins, sandboxed local MCP servers and sharing.

## T-0901 The rie CLI: login and status
- Version: 0.9.0
- Depends: T-0807
- Review: security
- Spec: docs/spec/api.md section 5
- Paths: crates/riehub-cli/, crates/riehub-server/, web/

### Build
1. New binary crate `riehub-cli`, binary `rie`. It depends on
   `riehub-core` only and speaks HTTP with `rustls`.
2. Device-style login: `POST /api/v1/device/start` returns a user code
   and a device code. The user approves in the web UI
   (`/device`, session required, shows the requested scopes). `rie`
   polls `POST /api/v1/device/token` and receives a hub key once.
   Codes expire in 10 minutes. Polling is rate limited.
3. Store the key in the OS keychain when available (`keyring` crate),
   else in `~/.config/rie/credentials` with mode 0600.
4. `rie status`, `rie logout` (revokes the key).
5. `rie` refuses to talk to a hub over plain HTTP unless the host is
   loopback.

### Accept
- `device_flow_issues_key_after_approval` (server test).
- `device_code_expires`, `device_code_is_single_use`,
  `unapproved_device_code_issues_nothing`.
- `user_code_guessing_is_throttled`.
- `credentials_file_is_0600`.
- `plain_http_to_remote_hub_is_refused`.
- `rie_status_prints_no_secret` (canary as the key).

### Proof
`cargo test -p riehub-cli && cargo test -p riehub-server device`

## T-0902 Agent config writer and profiles
- Version: 0.9.0
- Depends: T-0901
- Review: security
- Spec: docs/spec/api.md section 5
- Paths: crates/riehub-cli/src/agents/

### Build
1. One module per agent behind an `Agent` trait: detect, read current
   setting, apply `{ base_url, key_ref, model }`, undo. Start with
   Claude Code, Codex, Gemini CLI, OpenCode.
2. Edits change only the needed keys. JSON, JSONC and TOML files keep
   comments, key order and formatting (use `toml_edit` and a
   format-preserving JSON editor crate named in a new ADR).
3. Every write is atomic and keeps one timestamped backup next to the
   file. `rie undo <agent>` restores it.
4. The hub key is written as an environment variable reference or to
   the agent's own secret store when it has one, not inline, unless the
   agent supports nothing else. Say which in `rie use` output.
5. `rie agents`, `rie use`, `rie undo`, `rie profile save` and
   `rie profile apply`.
6. Fixture configs per agent under `tests/fixtures/agents/`.

### Accept
- Per agent: `apply_changes_only_the_expected_keys` (diff against the
  fixture), `comments_and_order_are_preserved`, `undo_restores_bytes`.
- `write_is_atomic` (a crash injected between temp write and rename
  leaves the original).
- `missing_agent_is_reported_not_created`.
- `profile_apply_is_idempotent`.

### Proof
`cargo test -p riehub-cli agents`

## T-0903 Library sync
- Version: 0.9.0
- Depends: T-0902
- Review: security
- Spec: docs/spec/api.md section 5, docs/spec/automation.md section 1
- Paths: crates/riehub-cli/src/sync/

### Build
1. `rie sync` pulls the user's instructions, skills and agent profiles
   with a `manage` key and writes them into each detected agent's
   format and directory. It also adds the hub's MCP gateway as an MCP
   server entry.
2. Files written by `rie` carry a marker so later syncs update or
   remove only what `rie` created. User-made files are never touched.
3. `rie sync --dry-run` prints the plan. `rie sync --push <dir>`
   uploads a local skill folder to the hub.
4. Extraction rules for skill files are the same as on the server.

### Accept
- `sync_writes_expected_tree_per_agent` (golden directories).
- `sync_twice_is_a_no_op`.
- `sync_removes_only_marked_files`.
- `user_file_with_same_name_is_not_overwritten`.
- `skill_with_dotdot_path_is_refused`.
- `dry_run_writes_nothing`.

### Proof
`cargo test -p riehub-cli sync`

## T-0904 Imports
- Version: 0.9.0
- Depends: T-0903
- Review: security
- Spec: docs/spec/api.md section 5
- Paths: crates/riehub-cli/src/import/

### Build
1. `rie import env`: reads known provider key variables from the
   environment or a named file and creates connections.
2. `rie import bifrost <config.json>`: providers and keys from a
   Bifrost `config.json`.
3. `rie import magpie <export>`: providers from a magpie export.
4. Each import shows a plan with credentials masked, asks for
   confirmation, and never writes a credential to disk or to its own
   output.
5. Fixtures are synthetic. Document the formats handled in
   `docs/guides/migrate.md`.

### Accept
- One golden test per source.
- `import_output_never_shows_a_credential` (canary).
- `unknown_fields_are_reported_not_fatal`.
- `import_is_idempotent_by_provider_and_fingerprint`.

### Proof
`cargo test -p riehub-cli import`

## T-0905 WebAssembly plugins
- Version: 0.9.0
- Depends: T-0807
- Review: security
- Spec: docs/adr/0014-wasm-plugins.md, docs/spec/governance.md section 5
- Paths: crates/riehub-plugins/, crates/riehub-router/, crates/riehub-server/, web/

### Build
1. New crate `riehub-plugins` on `wasmtime` with the component model.
   A WIT file defines `on-request`, `on-event`, `on-response` over the
   canonical model.
2. No WASI file system, no environment, no clock beyond a coarse
   monotonic one, no random beyond a seeded one unless requested.
   Network only through a host function that uses the guarded client
   and the manifest's approved host list.
3. Fuel and memory limits per call. A trap or limit is an error.
4. Admin installs (upload, manifest review, hash pinned). Users enable
   per hub key. `on_error = "reject"` by default.
5. Ship three example plugins in `examples/plugins/`: parameter
   override, model map, word guard. They are built in CI.
6. Plug into the pipeline `Transform` stage after masking.

### Accept
- `plugin_can_rewrite_a_request_parameter`.
- `plugin_without_network_grant_cannot_open_a_socket`.
- `plugin_infinite_loop_is_stopped_by_fuel`.
- `plugin_memory_bomb_is_stopped`.
- `plugin_trap_rejects_request_by_default`, `...skips_when_configured`.
- `plugin_runs_only_for_users_who_enabled_it`.
- `plugin_sees_masked_text_only`.
- `changed_plugin_bytes_do_not_load` (hash pin).

### Proof
`cargo test -p riehub-plugins`

## T-0906 Sandboxed stdio MCP servers
- Version: 0.9.0
- Depends: T-0807
- Review: security
- Spec: docs/spec/mcp.md section 6, docs/adr/0009-mcp-remote-first.md
- Paths: crates/riehub-mcp/src/stdio/, crates/riehub-server/

### Build
1. Enabled only when `mcp.stdio = true`, on Linux, and `bwrap` is
   present. Otherwise stdio catalog entries are shown as unavailable.
2. Commands come from admin-approved catalog entries: an absolute
   program path and fixed arguments. No user text reaches the command
   line. User configuration goes in as environment variables from an
   allowlist of names.
3. `bwrap` arguments: new user, pid, ipc, uts and cgroup namespaces,
   no network unless the entry asks for it, read-only bind of the
   program's directory, a private tmpfs, no data directory, die with
   parent.
4. Resource limits through `prlimit` (the `nix` crate) and a wall
   clock idle timeout. One process per user and connector.

### Accept
- `stdio_is_refused_when_disabled_or_not_linux`.
- `sandboxed_process_cannot_read_the_data_directory`.
- `sandboxed_process_has_no_network_by_default`.
- `user_text_never_reaches_argv`.
- `process_is_killed_after_idle_timeout_and_on_server_exit`.
- `two_users_get_two_processes`.

### Proof
`cargo test -p riehub-mcp stdio`

## T-0907 Sharing between users
- Version: 0.9.0
- Depends: T-0903
- Review: security
- Spec: docs/spec/automation.md section 1, docs/adr/0005-user-vault-on-cofferak.md
- Paths: crates/riehub-vault/, crates/riehub-automation/, crates/riehub-server/, web/

### Build
1. Each coffer gets a cofferak identity
   (`cofferak_core::identity::IdentitySecret::generate`), stored
   encrypted in the coffer. The public half is published on the user
   record.
2. Sharing a skill, instruction, agent profile or workflow copies it:
   the sender seals the item to the recipient's public identity with
   `identity::seal_transfer`. The recipient opens it with
   `open_transfer` on next login and stores their own copy.
3. Shares are copies, not live links. Credentials and connections can
   never be shared. Workflows are shared without grants.
4. Routes `shares` (Self): send, inbox, accept, decline.
5. The recipient sees the sender's verified identity fingerprint.

### Accept
- `shared_skill_arrives_and_opens_only_for_the_recipient`.
- `server_cannot_open_a_pending_share` (no key in the store opens it).
- `tampered_share_is_rejected`.
- `connection_or_credential_cannot_be_shared` (no route accepts those
  kinds, asserted on the enum).
- `shared_workflow_has_no_grant`.
- `declined_share_is_deleted`.

### Proof
`cargo test -p riehub-automation share`

## T-0999 Release 0.9.0
- Version: 0.9.0
- Depends: T-0901, T-0902, T-0903, T-0904, T-0905, T-0906, T-0907
- Review: standard
- Type: chore
- Spec: SDD.md section "Releases"
- Paths: Cargo.toml, web/package.json, CHANGELOG.md, README.md, docs/, .github/workflows/

### Build
1. Version `0.9.0` in `Cargo.toml` and `web/package.json`.
2. `CHANGELOG.md` section `## [0.9.0] - <today>`.
3. The release workflow also builds `rie` for the four targets.
4. Docs pages `docs/guides/rie-cli.md` and `docs/guides/plugins.md`,
   added to the nav.
5. After `finish`, run `factory/factory.sh release 0.9.0`.

### Accept
- `factory/factory.sh release 0.9.0` prints `RELEASED v0.9.0`.

### Proof
`make gate`
