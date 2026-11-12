# M04 OAuth connections and the MCP gateway - version 0.4.0

Users sign in to providers and tools with their own accounts, and
riehub becomes one MCP server per user.

## T-0401 OAuth client engine
- Version: 0.4.0
- Depends: T-0306
- Review: security
- Spec: docs/spec/mcp.md section 2, docs/spec/router.md section 4, docs/adr/0008-connection-auth-types.md
- Paths: crates/riehub-oauth/, crates/riehub-server/, Cargo.toml

### Build
1. New crate `riehub-oauth` (depends on vault, egress, store, crypto,
   core): authorization code with PKCE (S256), device code, refresh,
   revocation. Hand-written on the guarded client, no OAuth framework.
2. `POST /api/v1/oauth/start` (Self) returns the authorization URL.
   `GET /api/v1/oauth/callback` (Public, bound to the session by the
   `state` record) exchanges the code.
3. `state`: 32 random bytes, single use, 10 minutes, stored as a
   user-owned record with the PKCE verifier encrypted.
4. Token sets are written to the vault path the caller names. Refresh
   takes a store lease per token set so two nodes do not refresh at
   once, and writes the new set back.
5. Redirect URIs come from `server.public_url` only.

### Accept
- Against a mock authorization server: `code_flow_with_pkce_succeeds`,
  `device_flow_succeeds`.
- `state_is_single_use`, `state_expires`, `state_from_other_session_is_refused`.
- `missing_or_wrong_pkce_verifier_fails`.
- `refresh_rotates_tokens_in_vault`.
- `concurrent_refresh_happens_once`.
- `tokens_never_in_db_plain_or_logs` (canary as the access token).
- `authorization_server_on_private_address_is_refused`.

### Proof
`cargo test -p riehub-oauth`

## T-0402 OAuth and cloud connections for providers
- Version: 0.4.0
- Depends: T-0401
- Review: security
- Spec: docs/spec/router.md section 4, docs/adr/0008-connection-auth-types.md
- Paths: crates/riehub-router/, crates/riehub-server/, docs/guides/

### Build
1. Connection auth type `oauth2`: catalog entries may carry an
   `oauth` block (endpoints or discovery URL, scopes, client id, client
   secret reference into the instance coffer, `terms_url`,
   `terms_checked`). An entry without `terms_url` is rejected.
2. Auth type `cloud`: Google service account or workload identity for
   Vertex AI, AWS access keys or role for Bedrock (SigV4), Azure
   client credentials. Add the `bedrock` provider kind (Converse API).
3. The router refreshes an expired token inside the request.
4. Ship OAuth catalog entries only for Google (Vertex AI and Gemini
   with a client the admin registers). Document in
   `docs/guides/provider-sign-in.md` how an admin adds another
   provider and the rule of ADR 0008.

### Accept
- `oauth_connection_calls_provider_with_bearer_token`.
- `expired_token_is_refreshed_inside_request`.
- `catalog_entry_without_terms_url_is_rejected`.
- `bedrock_request_is_sigv4_signed` (against a mock that verifies the
  signature).
- `vertex_request_uses_service_account_token`.
- `cloud_credentials_are_only_in_vault`.

### Proof
`cargo test -p riehub-router connection_auth` (put the tests of this card in a module named `connection_auth`)

## T-0403 MCP client
- Version: 0.4.0
- Depends: T-0401
- Review: security
- Spec: docs/spec/mcp.md section 2, docs/adr/0009-mcp-remote-first.md
- Paths: crates/riehub-mcp/

### Build
1. New crate `riehub-mcp`, module `client`: JSON-RPC 2.0, Streamable
   HTTP and HTTP with SSE transports, `initialize`, `tools/list`,
   `tools/call`, `resources/list`, `resources/read`, `prompts/list`,
   `prompts/get`, `notifications/tools/list_changed`, ping, session id
   header handling.
2. Auth: none, static header, OAuth 2.1 with protected resource
   metadata discovery, authorization server metadata, dynamic client
   registration, using `riehub-oauth`.
3. Limits: message 8 MiB, call timeout 60 s, at most 1 000 tools per
   server.
4. A mock MCP server for tests, with both transports and OAuth.

### Accept
- `initialize_and_list_tools_over_streamable_http`, `...over_sse`.
- `call_tool_returns_content`.
- `oauth_discovery_and_dynamic_registration`.
- `oversized_message_is_rejected`, `slow_call_times_out`.
- `server_on_private_address_is_refused`.
- `malformed_json_rpc_does_not_panic` (property test).
- `tool_arguments_and_results_are_content` (canary).

### Proof
`cargo test -p riehub-mcp client`

## T-0404 Connector catalog and per-user connectors
- Version: 0.4.0
- Depends: T-0403
- Review: security
- Spec: docs/spec/mcp.md sections 1-2
- Paths: crates/riehub-mcp/, crates/riehub-server/

### Build
1. Catalog entries (instance records) with a starter file
   `crates/riehub-mcp/catalog/connectors.toml` holding only servers
   with a public, documented remote MCP endpoint. Each entry names its
   documentation URL.
2. Admin routes `admin/catalog/connectors`.
3. Connector record (user-owned): catalog id or custom URL, auth type,
   status, enabled tools. Label and URL are encrypted. Credential or
   token set at `mcp/<id>/credential`.
4. Routes `connectors` (Self): create, connect (starts OAuth when
   needed), list, get, enable or disable tools, refresh tool list,
   delete.
5. Tool list cache as an encrypted record.
6. Access profiles may restrict which catalog entries and whether
   custom URLs are allowed.

### Accept
- `connect_with_static_header`, `connect_with_oauth`.
- `two_users_same_catalog_entry_have_separate_credentials`.
- `user_cannot_see_or_use_another_users_connector`.
- `custom_url_forbidden_by_profile_is_refused`.
- `connector_credential_is_never_returned`.
- `needs_auth_status_after_token_revocation`.

### Proof
`cargo test -p riehub-mcp connector && cargo test -p riehub-server connectors`

## T-0405 MCP gateway
- Version: 0.4.0
- Depends: T-0404
- Review: security
- Spec: docs/spec/mcp.md section 3
- Paths: crates/riehub-mcp/src/gateway/, crates/riehub-server/

### Build
1. `POST /mcp` and `GET /mcp`, class `Key(Mcp)`, Streamable HTTP
   server side: `initialize`, `tools/list`, `tools/call`, resources,
   prompts.
2. Aggregation with names `<connector-slug>__<name>`.
3. Filtering as the intersection in the spec, computed in one function
   `visible_tools(key, user, virtual_mcp)`.
4. Forward calls with the user's credential. Apply the hub key's rate
   limit. Audit variant `McpToolCall { connector, tool_index, outcome,
   duration_ms }`.
5. Upstream failure of one connector does not break `tools/list` for
   the others. The failing one is marked in its status.

### Accept
- `gateway_lists_tools_of_all_connectors_with_prefixes`.
- `call_is_forwarded_to_the_right_connector_with_the_users_credential`.
- `tool_outside_allowlist_is_not_listed_and_not_callable`.
- `key_without_mcp_scope_is_403`.
- `one_broken_connector_does_not_break_the_list`.
- `tool_call_audit_event_has_no_arguments`.
- `user_a_key_never_reaches_user_b_connector`.
- The canary test covers tool arguments and results.

### Proof
`cargo test -p riehub-mcp gateway && cargo test -p riehub-server mcp`

## T-0406 Virtual MCPs
- Version: 0.4.0
- Depends: T-0405
- Review: standard
- Spec: docs/spec/mcp.md section 3
- Paths: crates/riehub-mcp/, crates/riehub-server/

### Build
1. Virtual MCP record (user-owned): slug lookup hash, tool list.
2. `/mcp/<slug>` serves only that bundle.
3. Routes `virtual-mcps` (Self). Admin-published bundles of catalog
   entries through access profiles.

### Accept
- `virtual_mcp_exposes_only_its_tools`.
- `unknown_slug_is_404`.
- `published_bundle_uses_each_users_own_connectors`.

### Proof
`cargo test -p riehub-mcp virtual`

## T-0407 Tools in the router and agent mode
- Version: 0.4.0
- Depends: T-0406
- Review: security
- Spec: docs/spec/mcp.md section 4
- Paths: crates/riehub-router/, crates/riehub-mcp/, crates/riehub-server/

### Build
1. Header `x-riehub-tools`: add the gateway's tool definitions to the
   canonical request (pipeline `Transform` stage).
2. Header `x-riehub-agent: auto`: execute tool calls on the hub key's
   auto-approve list and continue the loop, at most 10 steps. Other
   tool calls go back to the caller.
3. Streaming keeps working: tool execution appears to the caller as
   the surface's normal tool call events followed by more output.
4. Each step counts against limits and writes usage and audit events.

### Accept
- `tools_are_injected_for_each_surface`.
- `agent_mode_executes_only_auto_approved_tools`.
- `agent_mode_stops_at_max_steps`.
- `tool_result_cannot_extend_the_allowlist` (a result that asks for
  another tool by name changes nothing).
- `each_agent_step_is_metered_and_audited`.

### Proof
`cargo test -p riehub-server agent_mode`

## T-0408 Web: connectors and tools
- Version: 0.4.0
- Depends: T-0407, T-0307
- Review: standard
- Spec: docs/spec/web.md section 5
- Paths: web/

### Build
1. Tools area: Connectors gallery from the catalog, connect flow
   including the OAuth redirect and return, per-tool toggles, status.
   Virtual MCPs editor.
2. Hub key editor gains tool allowlist and auto-approve list.
3. "Use it" panel for MCP: the URL and a config snippet per agent.
4. Admin: Catalog connectors.
5. Playwright: connect a mock MCP server with OAuth, list tools
   through `/mcp` with a hub key, call one.

### Accept
- `make e2e` passes with the new journey.
- Accessibility check passes on every new page.

### Proof
`make web-test && make e2e`

## T-0499 Release 0.4.0
- Version: 0.4.0
- Depends: T-0401, T-0402, T-0403, T-0404, T-0405, T-0406, T-0407, T-0408
- Review: standard
- Type: chore
- Spec: SDD.md section "Releases"
- Paths: Cargo.toml, web/package.json, CHANGELOG.md, README.md, docs/

### Build
1. Version `0.4.0` in `Cargo.toml` and `web/package.json`.
2. `CHANGELOG.md` section `## [0.4.0] - <today>`.
3. New docs page `docs/guides/mcp-gateway.md`, added to the nav.
4. After `finish`, run `factory/factory.sh release 0.4.0`.

### Accept
- `factory/factory.sh release 0.4.0` prints `RELEASED v0.4.0`.

### Proof
`make gate`
