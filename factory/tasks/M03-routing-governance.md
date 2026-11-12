# M03 Routing and governance - version 0.3.0

Requests survive provider failures, and every level from instance to
hub key can be limited.

## T-0301 Routing groups, aliases and fallback
- Version: 0.3.0
- Depends: T-0210
- Review: standard
- Spec: docs/spec/router.md sections 3, 6
- Paths: crates/riehub-router/src/routing/, crates/riehub-server/

### Build
1. Routing group record (user-owned, encrypted data: name, targets,
   strategy, weights). Name lookup is the keyed hash. Aliases are
   groups with one target.
2. Strategies `order`, `rotate`, `weighted`. Nesting up to 3 levels,
   cycles rejected on save.
3. Fallback on the retryable error classes, at most 3 attempts,
   never after the first event reached the caller.
4. Model addressing step 1 of the spec. `GET /v1/models` and the Gemini
   model list include groups.
5. Routes `routing-groups` (Self). The usage row records `attempts`.

### Accept
- `order_uses_first_target_then_falls_back_on_overload`.
- `no_fallback_on_bad_request_or_content_filter`.
- `no_fallback_after_first_streamed_event`.
- `rotate_cycles_targets`, `weighted_respects_weights` (statistical,
  fixed seed).
- `nested_group_resolves`, `cycle_is_rejected_on_save`.
- `auth_error_marks_connection_needs_attention`.
- `attempts_are_recorded`.

### Proof
`cargo test -p riehub-router routing`

## T-0302 Health, circuit breaker and smart strategy
- Version: 0.3.0
- Depends: T-0301
- Review: standard
- Spec: docs/spec/router.md section 6
- Paths: crates/riehub-router/src/routing/

### Build
1. Per-target health in memory: rolling error rate, latency
   percentiles, last error class.
2. Circuit breaker: open after 5 failures in 30 s, half-open probe
   after 30 s, close on success. Uses the `Clock` trait.
3. Strategy `smart`: score = weighted health, latency and error rate.
   Open targets are skipped.
4. `GET /api/v1/routing-groups/{id}/status` (Self): live state per
   target.

### Accept
- `breaker_opens_after_5_failures_in_window`.
- `breaker_half_opens_then_closes_on_success`,
  `breaker_reopens_on_probe_failure`.
- `smart_prefers_healthy_low_latency_target`.
- `all_targets_open_returns_surface_overloaded_error`.

### Proof
`cargo test -p riehub-router routing::health`

## T-0303 Quota windows, usage and pace strategies
- Version: 0.3.0
- Depends: T-0302
- Review: standard
- Spec: docs/spec/router.md section 6
- Paths: crates/riehub-router/src/routing/, crates/riehub-router/src/quota.rs

### Build
1. Quota windows per connection: a user-set limit (requests or tokens
   per window of 5 hours, day, week, month), plus what provider
   response headers report (`x-ratelimit-*`, `anthropic-ratelimit-*`,
   `retry-after`). Counters in the store.
2. Strategy `usage`: most quota left. Strategy `pace`: prefers the
   target whose use is furthest below the straight line to its reset.
3. A `quota` error marks the window exhausted until its reset.
4. `GET /api/v1/connections/{id}/quota` (Self).

### Accept
- `usage_picks_target_with_most_quota_left`.
- `pace_spreads_use_over_the_window` (simulation with TestClock).
- `quota_error_exhausts_window_until_reset`.
- `header_reported_limits_update_the_window`.
- `two_nodes_share_quota_counters` (two router instances on one store).

### Proof
`cargo test -p riehub-router quota`

## T-0304 Sticky conversations
- Version: 0.3.0
- Depends: T-0302
- Review: security
- Spec: docs/spec/router.md section 6
- Paths: crates/riehub-router/src/routing/sticky.rs

### Build
1. Conversation key: `hmac(per-process random key, hub_key_id ||
   sha384(first user message))`. The key and the map live in memory
   only, with a 1 hour idle expiry and a size cap.
2. A sticky target is used while it is healthy and still in the group.
3. Opt out per group (`sticky: false`).

### Accept
- `same_conversation_stays_on_one_target`.
- `sticky_target_is_dropped_when_breaker_opens`.
- `sticky_map_holds_no_message_text` (canary).
- `sticky_map_is_bounded`.

### Proof
`cargo test -p riehub-router sticky`

## T-0305 Rate limits and budgets
- Version: 0.3.0
- Depends: T-0210
- Review: security
- Spec: docs/spec/governance.md sections 1-2
- Paths: crates/riehub-governance/, crates/riehub-router/, crates/riehub-server/

### Build
1. New crate `riehub-governance`. Token bucket rate limits (requests
   and tokens per minute) and period budgets on store counters.
2. Levels for this task: instance, user, hub key. Teams come in
   T-0306. A request passes every level that applies.
3. Plug into the pipeline's `Admission` stage. Refusals use the
   surface's own error shape and `Retry-After`.
4. Charge tokens and cost after the response, including cancelled
   streams.
5. Counter store error means refusal.
6. Routes `limits` (Self) for the user's keys, `admin/limits`
   (AdminMeta) for instance and user levels. Audit variants
   `LimitUpdate`, `RequestRefused { level, reason }`.

### Accept
- `rpm_limit_refuses_with_429_and_retry_after`.
- `tpm_limit_counts_tokens_after_response`.
- `budget_refuses_when_spent`, `budget_resets_at_period_start`.
- `lowest_refusing_level_is_named`.
- `counter_error_refuses_request` (fail closed).
- `two_nodes_share_limits`.
- `refusal_uses_each_surfaces_error_shape` (4 surfaces).

### Proof
`cargo test -p riehub-governance && cargo test -p riehub-server limits`

## T-0306 Teams, shared connections and access profiles
- Version: 0.3.0
- Depends: T-0305
- Review: security
- Spec: docs/spec/governance.md sections 1, 3, docs/spec/router.md section 4, docs/spec/security.md section 5
- Paths: crates/riehub-governance/, crates/riehub-router/, crates/riehub-server/

### Build
1. Teams (instance records): name, members, managers, primary team per
   user. Team level in limits and in usage rows.
2. Shared connections: credential in the instance coffer at
   `shared/<id>/credential`. Write-only, like user credentials.
3. Access profiles as specified. Resolution: allowlists intersect, the
   lowest limit wins. One function, `effective_policy(user)`.
4. The router offers a shared connection to a user only when the
   effective policy allows it and its model.
5. Admin routes `admin/teams`, `admin/shared-connections`,
   `admin/profiles`.

### Accept
- `user_without_profile_cannot_use_shared_connection`.
- `profile_allows_only_listed_models`.
- `two_profiles_intersect`, `lowest_limit_wins`.
- `shared_credential_is_never_returned`.
- `admin_who_created_a_shared_connection_cannot_read_requests_through_it`
  (the admin usage view holds metadata only, and the canary test sends
  a canary through a shared connection).
- `team_budget_refuses_member_requests`.
- `profile_can_forbid_own_connections_of_a_provider_kind`.

### Proof
`cargo test -p riehub-governance profile && cargo test -p riehub-server shared`

## T-0307 Web: routing, limits and admin console
- Version: 0.3.0
- Depends: T-0303, T-0304, T-0306, T-0211
- Review: standard
- Spec: docs/spec/web.md section 5
- Paths: web/

### Build
1. Router area: Routing groups (drag to order targets, strategy
   picker, live status with breaker state).
2. Usage area: Limits (the user's keys and budgets). The dashboard
   blocks for budget and quota now show real data.
3. Admin: Teams, Policies (access profiles), Shared connections,
   instance limits.
4. Playwright: build a group of two mock upstreams, make the first
   fail, see the fallback in Activity. Set a tiny budget, see the
   refusal.

### Accept
- `make e2e` passes with the new journeys.
- Accessibility check passes on every new page.
- Drag to order has a keyboard alternative (move up and down buttons).

### Proof
`make web-test && make e2e`

## T-0399 Release 0.3.0
- Version: 0.3.0
- Depends: T-0301, T-0302, T-0303, T-0304, T-0305, T-0306, T-0307
- Review: standard
- Type: chore
- Spec: SDD.md section "Releases"
- Paths: Cargo.toml, web/package.json, CHANGELOG.md, README.md, docs/

### Build
1. Version `0.3.0` in `Cargo.toml` and `web/package.json`.
2. `CHANGELOG.md` section `## [0.3.0] - <today>`.
3. New docs pages `docs/guides/routing.md` and
   `docs/guides/limits-and-teams.md`, added to the nav.
4. After `finish`, run `factory/factory.sh release 0.3.0`.

### Accept
- `factory/factory.sh release 0.3.0` prints `RELEASED v0.3.0`.

### Proof
`make gate`
