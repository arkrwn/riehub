# M06 Privacy and guardrails - version 0.6.0

Sensitive values are caught before they leave, rules can block
requests, and a few opt-in conveniences arrive.

## T-0601 Masking engine
- Version: 0.6.0
- Depends: T-0507
- Review: security
- Spec: docs/spec/governance.md section 4
- Paths: crates/riehub-governance/src/masking/, crates/riehub-router/src/model/

### Build
1. Detectors as specified, each a module with its own tests: provider
   API keys, cloud keys, private key blocks, JWTs, bearer tokens,
   emails, phone numbers, payment cards with Luhn, IPv4 and IPv6.
   Use the `regex` crate with a compiled size limit.
2. `mask(parts, rules) -> Mapping` over every text part of a request,
   including tool arguments and results. `riehub-governance` must not
   depend on `riehub-router`: define a `TextParts` visitor trait here,
   and implement it for `ChatRequest` and `ChatEvent` in
   `riehub-router`. Placeholders `[[KIND_N]]`, the same value gets the same
   placeholder inside one request.
3. `Restorer` for streams: buffers a possible partial placeholder
   across chunk boundaries, restores in text and in tool call
   arguments.
4. `Mapping` lives in memory for the request and is zeroized.
5. Test fixtures build fake secrets by concatenation so the rules lint
   stays green.

### Accept
- One table test per detector with positives and negatives, including
  near misses (a 16-digit number that fails Luhn is not a card).
- `mask_then_restore_is_identity` (property test over random texts
  with planted values).
- `restore_across_every_possible_chunk_split` (property test).
- `same_value_same_placeholder`.
- `user_regex_over_size_limit_is_rejected`.
- `pathological_input_completes_in_linear_time` (1 MiB of `a`s under
  a time bound).
- `mapping_is_not_debug_printable`.

### Proof
`cargo test -p riehub-governance masking`

## T-0602 Privacy rules, guardrails and the masking log
- Version: 0.6.0
- Depends: T-0601
- Review: security
- Spec: docs/spec/governance.md sections 4-5
- Paths: crates/riehub-governance/, crates/riehub-router/, crates/riehub-server/

### Build
1. Rule records (user-owned, encrypted for custom patterns): detector
   or pattern, action `mask`, `warn` or `block`, scope (all requests or
   named hub keys).
2. Plug masking and guardrails into the pipeline `Transform` stage,
   before target selection. Restore on the way back.
3. Guardrail rule types of this version: detectors with `block` or
   `warn`, custom regex, maximum input size, allowed file types,
   allowed tool names.
4. Failure in detection or restore refuses with `503`.
5. Masking log (user-owned, encrypted): rule id, kind, length, count.
   Routes `privacy/rules`, `privacy/log`, `POST privacy/preview` (runs
   masking on a sample and returns the masked text, nothing is sent
   upstream or stored).
6. Baseline rules in access profiles. Admin sees counts per rule and
   user through `admin/usage`.
7. Audit variant `GuardrailBlock { rule }`.

### Accept
- `masked_value_never_reaches_upstream` (the mock upstream asserts the
  canary is absent and the placeholder present).
- `reply_is_restored_for_each_surface_streaming_and_not`.
- `block_rule_refuses_with_content_filter_error`.
- `warn_rule_counts_but_sends`.
- `masking_error_refuses_request_with_503` (inject a fault).
- `masking_log_has_no_values` (canary).
- `baseline_rule_from_profile_cannot_be_disabled_by_user`.
- `admin_sees_counts_only`.
- `preview_sends_nothing_upstream`.

### Proof
`cargo test -p riehub-server privacy`

## T-0603 Intent routing
- Version: 0.6.0
- Depends: T-0602, T-0303
- Review: standard
- Spec: docs/spec/router.md section 6
- Paths: crates/riehub-router/src/routing/intent.rs

### Build
1. Per-group opt-in: a classifier target (a model of the user), a list
   of labels with descriptions, and a label to target map.
2. The classifier sees the last user message, after masking, with a
   fixed prompt, and must answer with one label. Anything else falls
   back to the group's normal strategy.
3. Classifier calls are metered and audited like any request, with a
   short timeout (3 s) and no fallback chain of their own.
4. Sticky conversations keep the first label's target.

### Accept
- `label_maps_to_target`.
- `unknown_label_or_timeout_falls_back_to_strategy`.
- `classifier_receives_masked_text`.
- `classifier_call_is_metered`.

### Proof
`cargo test -p riehub-router intent`

## T-0604 Response cache
- Version: 0.6.0
- Depends: T-0602
- Review: security
- Spec: docs/spec/governance.md section 6
- Paths: crates/riehub-router/src/cache.rs

### Build
1. Opt in per hub key with a time to live. Key: `hmac(content_key
   ("cache"), canonical bytes of the request without metadata)`.
2. Entries are user-owned encrypted records. Only successful,
   complete, non-tool responses are stored. Cached replies are replayed
   as a stream when the caller asked for one.
3. Response header `x-riehub-cache: hit` or `miss`. Usage rows mark
   cache hits with zero cost.
4. A background sweep deletes expired entries using `sort` as expiry.

### Accept
- `second_identical_request_is_a_hit_and_does_not_call_upstream`.
- `different_user_same_request_is_a_miss`.
- `different_hub_key_of_same_user_without_opt_in_is_a_miss`.
- `expired_entry_is_a_miss`.
- `cached_reply_is_encrypted_at_rest` (canary).
- `errors_and_tool_calls_are_not_cached`.

### Proof
`cargo test -p riehub-router cache`

## T-0605 Built-in web tools
- Version: 0.6.0
- Depends: T-0405
- Review: security
- Spec: docs/spec/mcp.md section 5
- Paths: crates/riehub-mcp/src/builtin/

### Build
1. `web_fetch`: guarded client, GET only, `http` and `https` only,
   4 MiB cap, 20 s timeout, content types text and HTML only. HTML to
   text with the `html2text` crate, recorded in a new ADR.
2. `web_search`: providers behind one trait, starting with two the
   user can bring a key for. Keys at `tool/<name>/key`.
3. Both appear in the gateway as a built-in connector the user can
   enable, with per-tool toggles like any connector.

### Accept
- `web_fetch_refuses_private_addresses_and_redirects_to_them`.
- `web_fetch_caps_size_and_time`.
- `web_fetch_refuses_non_text_content`.
- `web_search_uses_the_users_key_from_the_vault`.
- `builtin_tools_respect_the_tool_allowlist`.

### Proof
`cargo test -p riehub-mcp builtin`

## T-0606 Web: privacy
- Version: 0.6.0
- Depends: T-0603, T-0604, T-0605
- Review: standard
- Spec: docs/spec/web.md section 5
- Paths: web/

### Build
1. Router area, Privacy page: rules with Mask, Warn, Block, a "Try it"
   preview box, the masking log, cache and intent routing settings.
2. Tools area: the built-in web tools with key entry.
3. Playwright: enable email masking, send a request with an email
   through a mock upstream that echoes its input, assert the reply the
   caller sees contains the email and the upstream saw a placeholder.

### Accept
- `make e2e` passes with the new journey.
- Accessibility check passes on every new page.

### Proof
`make web-test && make e2e`

## T-0699 Release 0.6.0
- Version: 0.6.0
- Depends: T-0601, T-0602, T-0603, T-0604, T-0605, T-0606
- Review: standard
- Type: chore
- Spec: SDD.md section "Releases"
- Paths: Cargo.toml, web/package.json, CHANGELOG.md, README.md, docs/

### Build
1. Version `0.6.0` in `Cargo.toml` and `web/package.json`.
2. `CHANGELOG.md` section `## [0.6.0] - <today>`.
3. New docs page `docs/guides/privacy.md`, added to the nav.
4. After `finish`, run `factory/factory.sh release 0.6.0`.

### Accept
- `factory/factory.sh release 0.6.0` prints `RELEASED v0.6.0`.

### Proof
`make gate`
