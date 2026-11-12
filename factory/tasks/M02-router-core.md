# M02 Router core - version 0.2.0

A user connects a provider with an API key and calls it through any of
the four API formats, with usage recorded and a dashboard to see it.

## T-0201 Canonical request model
- Version: 0.2.0
- Depends: T-0106
- Review: security
- Spec: docs/spec/router.md section 1, docs/adr/0007-canonical-request-model.md
- Paths: crates/riehub-router/

### Build
1. New crate `riehub-router`, module `model`: the types of spec
   section 1. All text and bytes are `Content`.
2. `fold(events) -> ChatResponse` builds a full response from a stream.
3. `PassThrough` keyed by surface, holding `serde_json::Value` wrapped
   so that `Debug` prints `[content]`.
4. Size limits: at most 2 000 messages, 256 tools, 32 MiB total.

### Accept
- `fold_of_text_stream_equals_expected_response`.
- `fold_handles_interleaved_tool_calls`.
- `request_over_limits_is_invalid`.
- `debug_of_request_contains_no_message_text` (format a request holding
  a canary, assert it is absent).
- Property test: any generated event stream folds without panic.

### Proof
`cargo test -p riehub-router model`

## T-0202 Guarded HTTP client
- Version: 0.2.0
- Depends: T-0004
- Review: security
- Spec: docs/spec/security.md section 7 (S11)
- Paths: crates/riehub-egress/, Cargo.toml, scripts/rules-lint.sh

### Build
1. New crate `riehub-egress` (depends on `riehub-core` only).
   `GuardedClient` on `reqwest` with `rustls` and `aws-lc-rs`, TLS 1.3
   minimum, no proxy from environment, redirects handled by hand.
2. A custom DNS resolver step: resolve, reject every address in the
   blocked ranges of the spec for IPv4 and IPv6 (including
   IPv4-mapped IPv6), then connect to exactly the checked address.
3. `egress.allow` entries (host, host:port or CIDR) skip the range
   check. Plain `http://` is allowed only for allowlisted hosts.
4. Each redirect hop repeats the check. At most 5 hops. Credentials
   are not forwarded across hosts.
5. Timeouts: connect 10 s, first byte 120 s, idle 300 s. Response size
   caps are set by the caller.
6. This type is the only way to make an outbound request. Add a rules
   lint line that rejects `reqwest::Client::` and `hyper::Client`
   outside this module.

### Accept
- `blocks_loopback_private_linklocal_metadata_v4_and_v6` (table test,
  including `169.254.169.254`, `::1`, `fd00::/8`, `::ffff:127.0.0.1`,
  `100.64.0.0/10`, `0.0.0.0`).
- `blocks_hostname_that_resolves_to_private_address`.
- `blocks_redirect_to_private_address`.
- `dns_rebinding_second_answer_is_not_used` (resolver stub that changes
  its answer).
- `allowlisted_loopback_is_reachable`.
- `plain_http_to_public_host_is_refused`.
- `authorization_header_is_dropped_on_cross_host_redirect`.

### Proof
`cargo test -p riehub-egress`

## T-0203 Provider catalog and API key connections
- Version: 0.2.0
- Depends: T-0201, T-0202
- Review: security
- Spec: docs/spec/router.md section 4, docs/adr/0008-connection-auth-types.md
- Paths: crates/riehub-router/, crates/riehub-server/

### Build
1. Provider definitions as instance records, with a built-in catalog
   file `crates/riehub-router/catalog/providers.toml`: OpenAI,
   Anthropic, Google AI Studio, DeepSeek, Mistral, Groq, OpenRouter,
   xAI, Ollama (auth `none`). Each: kind, base URL, auth types, model
   source, price table with currency.
2. Admin routes `admin/catalog/providers` (AdminMeta) to add, edit and
   disable definitions.
3. Connection record (user-owned): provider id and status in `meta`.
   Label, slug and optional base URL override in encrypted `data`. Slug
   lookup is the keyed hash. Credential at vault path
   `conn/<id>/credential`.
4. Routes `connections` (Self): create, list, get, update, delete,
   `POST connections/{id}/test` (calls `models` on the provider).
5. A credential is write-only: no route returns it. Responses carry a
   fingerprint: the first 8 hex characters of `sha384` over the vault
   entry's ciphertext. Never a hash of the credential itself.
6. Audit variants `ConnectionCreate`, `ConnectionDelete`,
   `ConnectionTest`.

### Accept
- `create_connection_stores_credential_only_in_vault`: scan the
  database for the fake key.
- `credential_is_never_returned` (every connection route).
- `other_user_gets_404_for_connection_id`.
- `owner_cannot_list_another_users_connections`.
- `custom_base_url_to_private_address_is_refused`.
- `delete_removes_vault_entry`.
- `slug_lookup_is_not_stored_in_plain`: the slug text is absent from
  the database file.
- The canary test covers the credential and the label.

### Proof
`cargo test -p riehub-router connection && cargo test -p riehub-server`

## T-0204 OpenAI-compatible provider adapter and mock upstream
- Version: 0.2.0
- Depends: T-0203
- Review: standard
- Spec: docs/spec/router.md section 5
- Paths: crates/riehub-router/src/provider/, crates/riehub-router/tests/

### Build
1. `Provider` trait and `ProviderError` with classes as specified.
2. `provider::openai`: Chat Completions upstream, streaming with SSE,
   tools, images, `usage` with `stream_options.include_usage`,
   `models`, `embed`.
3. A robust SSE parser module shared by all adapters: handles split
   chunks, comments, multi-line `data`, `[DONE]`.
4. Test support crate or module `mock_upstream`: a local HTTP server
   that replays fixture files from `tests/fixtures/<provider>/` and
   records what it received. Fixtures are hand-written, small, and
   contain no real data.
5. Map HTTP status and error bodies to error classes.

### Accept
- `chat_non_streaming_maps_to_events`.
- `chat_streaming_text`, `chat_streaming_tool_calls`,
  `chat_streaming_with_usage`.
- `sse_parser_handles_arbitrary_chunk_boundaries` (property test that
  splits a fixture at random points).
- `status_401_is_auth`, `429_is_rate_limited_with_retry_after`,
  `529_or_503_is_overloaded`, `400_is_bad_request`, `timeout_is_timeout`.
- `credential_goes_only_to_the_configured_host`.
- `provider_error_text_is_content`.

### Proof
`cargo test -p riehub-router provider::openai`

## T-0205 OpenAI Chat Completions surface
- Version: 0.2.0
- Depends: T-0204
- Review: security
- Spec: docs/spec/router.md sections 2-3, docs/SPEC.md section 3.2
- Paths: crates/riehub-router/src/surface/, crates/riehub-server/

### Build
1. `Surface` trait: decode request, encode response, encode stream
   event, encode error. `surface::openai_chat` implements it.
2. Routes `POST /v1/chat/completions`, `GET /v1/models`,
   class `Key(Ai)`.
3. Model addressing step 2 and 3 of the spec (connection slug prefix,
   bare model id). Groups come in M03.
4. The request pipeline of SPEC section 3.2 as one function with
   explicit stages. Stages that do not exist yet are no-ops behind a
   trait so later tasks plug in: `Admission`, `Transform`, `Target`.
5. Audit variant `AiRequest { surface, provider_kind, catalog_model,
   stream }`, written before the upstream call.
6. Client disconnect cancels the upstream request.

### Accept
- End to end with the mock upstream: `chat_completion_round_trip`,
  `streaming_round_trip_is_valid_sse`.
- `unknown_model_is_openai_shaped_404`.
- `no_key_is_openai_shaped_401`.
- `audit_event_is_written_before_upstream_is_called` (the mock records
  call order through a shared counter).
- `request_is_refused_when_audit_write_fails`.
- `client_disconnect_cancels_upstream`.
- `upstream_error_body_is_not_logged` (canary in the error fixture).
- The canary test covers prompts and completions on this surface.

### Proof
`cargo test -p riehub-server chat_completions`

## T-0206 Anthropic adapter and Messages surface
- Version: 0.2.0
- Depends: T-0205
- Review: standard
- Spec: docs/spec/router.md sections 2, 5
- Paths: crates/riehub-router/src/provider/anthropic.rs, crates/riehub-router/src/surface/anthropic.rs, crates/riehub-server/

### Build
1. `provider::anthropic`: Messages API, streaming events, tools,
   images, thinking blocks with signatures, cache usage counters,
   `anthropic-version` header.
2. `surface::anthropic`: `POST /v1/messages`,
   `POST /v1/messages/count_tokens`. Accept `x-api-key` and bearer.
   Errors in Anthropic's shape.
3. `count_tokens` forwards to the provider when the target is
   Anthropic, otherwise answers with an estimate flagged in a response
   header `x-riehub-estimate: true`.

### Accept
- `messages_round_trip`, `messages_streaming_event_order_is_valid`
  (message_start, content_block_start, deltas, content_block_stop,
  message_delta, message_stop).
- `tool_use_and_tool_result_round_trip`.
- `thinking_blocks_keep_signature_on_same_format_path`.
- `cache_usage_is_reported`.
- `error_shape_is_anthropic`.
- The canary test covers this surface.

### Proof
`cargo test -p riehub-router anthropic && cargo test -p riehub-server messages`

## T-0207 Gemini adapter and surface
- Version: 0.2.0
- Depends: T-0205
- Review: security
- Spec: docs/spec/router.md sections 2, 5, docs/spec/identity.md section 4
- Paths: crates/riehub-router/src/provider/gemini.rs, crates/riehub-router/src/surface/gemini.rs, crates/riehub-server/

### Build
1. `provider::gemini`: `generateContent`, `streamGenerateContent`
   (SSE with `alt=sse`), function calling, inline data, usage metadata,
   `countTokens`, `embedContent`, model list.
2. `surface::gemini`: the routes in the spec. The model and the action
   are in the path (`{model}:generateContent`).
3. Accept the key in `x-goog-api-key` or `?key=`. The upstream key is
   always sent in the header, never in the URL.

### Accept
- `generate_content_round_trip`, `stream_generate_content_round_trip`.
- `function_call_round_trip`.
- `key_in_query_authenticates`.
- `query_string_never_appears_in_logs` (canary as a query value).
- `upstream_request_has_no_key_in_url`.
- `error_shape_is_gemini`.
- The canary test covers this surface.

### Proof
`cargo test -p riehub-router gemini && cargo test -p riehub-server gemini`

## T-0208 OpenAI Responses surface
- Version: 0.2.0
- Depends: T-0205
- Review: standard
- Spec: docs/spec/router.md section 2
- Paths: crates/riehub-router/src/surface/openai_responses.rs, crates/riehub-router/src/provider/openai.rs, crates/riehub-server/

### Build
1. `surface::openai_responses`: `POST /v1/responses`, stateless use
   only: `input` items, `instructions`, tools, streaming events
   (`response.created`, `response.output_text.delta`,
   `response.function_call_arguments.delta`, `response.completed`).
2. `previous_response_id` and `store: true` are refused with a clear
   error: riehub does not keep responses on the server.
3. Upstream Responses support in `provider::openai` for definitions
   that declare it. Otherwise translate to Chat Completions.
4. Reasoning items pass through on the same-format path.

### Accept
- `responses_round_trip`, `responses_streaming_event_order_is_valid`.
- `function_call_round_trip`.
- `previous_response_id_is_refused_with_400`.
- `responses_to_chat_completions_upstream_translation`.
- The canary test covers this surface.

### Proof
`cargo test -p riehub-router responses && cargo test -p riehub-server responses`

## T-0209 Cross-format matrix
- Version: 0.2.0
- Depends: T-0206, T-0207, T-0208
- Review: standard
- Type: test
- Spec: docs/adr/0007-canonical-request-model.md
- Paths: crates/riehub-router/tests/matrix.rs, crates/riehub-router/tests/fixtures/, docs/spec/router.md

### Build
1. A table-driven test: each of the 4 surfaces against each of the 3
   provider kinds, for the cases text, system prompt, multi-turn, tool
   call, tool result, image input, reasoning, streaming, stop reasons,
   usage.
2. Same-format identity: decode then encode a surface fixture gives a
   semantically equal JSON document.
3. For each cross-format cell assert the documented behaviour: full,
   degraded (say how), or refused with a clear error.
4. Generate the table into `docs/spec/router.md` section "Translation
   matrix" from the test's own table (a test that fails when the doc
   table is stale).

### Accept
- `matrix_all_cells_have_a_declared_outcome`.
- `matrix_cells_behave_as_declared`.
- `same_format_identity_holds_for_all_fixtures`.
- `doc_matrix_is_current`.

### Proof
`cargo test -p riehub-router --test matrix`

## T-0210 Usage metering and cost
- Version: 0.2.0
- Depends: T-0206
- Review: security
- Spec: docs/spec/router.md section 7, docs/spec/storage.md section 3
- Paths: crates/riehub-router/src/usage.rs, crates/riehub-store/, crates/riehub-server/

### Build
1. Write the usage row and update hourly buckets after each request,
   also for failures and cancelled streams (with what was counted).
2. Cost from the catalog price table. Per-connection price override in
   the connection's encrypted data.
3. When a provider reports no usage, estimate tokens with a simple
   character-based estimator and mark the row `estimated`.
4. `GET /api/v1/usage` (Self): totals and series grouped by a fixed
   enum set. `GET /api/v1/usage/requests`: recent rows of the caller.
5. `GET /api/v1/admin/usage` (AdminMeta, `usage:read:all`): the same
   without `connection_id` and `hub_key_id`.

### Accept
- `usage_row_written_for_success_error_and_cancel`.
- `cost_is_tokens_times_price`.
- `usage_row_has_no_content` (struct has only the spec fields, and the
  canary test extends to the usage tables).
- `user_cannot_read_another_users_usage`.
- `admin_usage_has_no_connection_or_key_ids`.
- `buckets_match_rows` (property test over random rows).

### Proof
`cargo test -p riehub-router usage && cargo test -p riehub-server usage`

## T-0211 Web: dashboard, connections and keys
- Version: 0.2.0
- Depends: T-0210, T-0109
- Review: standard
- Spec: docs/spec/web.md sections 4-5
- Paths: web/

### Build
1. Components: `StatCard`, `HeroStatCard`, `ProgressBar`, `StackedBars`,
   `Sparkline`, `SearchInput`, `CodeBlock`.
2. Overview dashboard exactly as the mapping table in the spec, with
   real data. Blocks whose data arrives in a later version (budget,
   quota) show an `EmptyState` that says so.
3. Router area: Connections (cards, add dialog with provider picker,
   test button), Models, Hub keys with a "use it" panel that shows the
   base URL and a `curl` example for each surface.
4. Usage area: Usage and cost (period picker, series, breakdown by
   model), Activity (recent requests table).
5. Admin: Catalog providers, Usage.
6. Playwright: add a connection against a mock upstream started by the
   test, call `/v1/chat/completions` with a new hub key, see the
   request on the dashboard and in Activity.

### Accept
- `make e2e` passes with the new journey.
- The dashboard matches the layout of the reference at 1440 px wide:
  three rows, cards with 24 px radius, pill navigation. Screenshots in
  both themes are saved under `web/e2e/__shots__`.
- Accessibility check passes on every new page.

### Proof
`make web-test && make e2e`

## T-0299 Release 0.2.0
- Version: 0.2.0
- Depends: T-0201, T-0202, T-0203, T-0204, T-0205, T-0206, T-0207, T-0208, T-0209, T-0210, T-0211
- Review: standard
- Type: chore
- Spec: SDD.md section "Releases"
- Paths: Cargo.toml, web/package.json, CHANGELOG.md, README.md, docs/, scripts/quickstart-check.sh

### Build
1. Version `0.2.0` in `Cargo.toml` and `web/package.json`.
2. `CHANGELOG.md` section `## [0.2.0] - <today>`.
3. README Quickstart now ends with a real model call through a hub
   key. Update `scripts/quickstart-check.sh` to do the same against the
   mock upstream. Add a docs page `docs/guides/connect-an-agent.md`
   with the base URL and environment variables for Claude Code, Codex
   and Gemini CLI, and add it to the nav.
4. After `finish`, run `factory/factory.sh release 0.2.0`.

### Accept
- `scripts/quickstart-check.sh` exits 0.
- `factory/factory.sh release 0.2.0` prints `RELEASED v0.2.0`.

### Proof
`make gate`
