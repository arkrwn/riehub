# M10 Hardening - version 1.0.0

No new features. Prove the promises, freeze the interfaces, and hand a
release candidate to the maintainer.

## T-1001 Invariant verification matrix
- Version: 1.0.0
- Depends: T-0999
- Review: security
- Type: test
- Spec: docs/SPEC.md section 2, docs/spec/security.md
- Paths: crates/riehub-server/tests/invariants/, docs/security-proof.md, mkdocs.yml

### Build
1. One test module per invariant S1 to S12. Each module lists the
   tests that prove the invariant, by name, including tests in other
   crates.
2. Fill the gaps: for every invariant, at least one test per surface
   and feature it applies to. Use the route table and the record kind
   enum to generate cases so nothing is missed by hand.
3. Extend the privacy canary test to every surface, every record kind
   and every export that exists.
4. Generate `docs/security-proof.md`: a table of invariant, what it
   means, and the tests that prove it. A test fails when the page is
   stale.

### Accept
- `every_invariant_has_tests_for_every_applicable_surface`.
- `every_record_kind_with_user_data_is_ciphertext_at_rest` (generated
  over the kind enum, with the canary).
- `every_self_route_isolates_users` and
  `every_admin_route_is_content_free` (generated over the table).
- `every_check_fails_closed` (fault injection on each admission and
  transform stage).
- `security_proof_page_is_current`.

### Proof
`cargo test -p riehub-server --test invariants`

## T-1002 Fuzzing
- Version: 1.0.0
- Depends: T-1001
- Review: security
- Type: test
- Spec: docs/spec/router.md, docs/spec/mcp.md, docs/spec/automation.md
- Paths: fuzz/, Makefile, .github/workflows/

### Build
1. `cargo-fuzz` targets for every parser of external input: the four
   surface decoders, the SSE parser, provider response decoders, MCP
   JSON-RPC frames, the workflow YAML parser and expression evaluator,
   the skill archive extractor, token parsing, the config loader, the
   masking restorer.
2. A seed corpus from the test fixtures.
3. `make fuzz-smoke` runs each target for 60 seconds. A nightly CI
   job runs each for 15 minutes and uploads crashes.
4. Every crash found is fixed with a regression test in the normal
   suite.

### Accept
- `make fuzz-smoke` finishes with no crash.
- Each target has at least 20 seed inputs.
- Any fix made here has a named regression test.

### Proof
`make fuzz-smoke`

## T-1003 Load and soak
- Version: 1.0.0
- Depends: T-1001
- Review: standard
- Type: test
- Spec: docs/spec/deploy.md section 6
- Paths: bench/, docs/benchmarks.md, mkdocs.yml

### Build
1. A Rust load tool in `bench/` that drives the four surfaces against
   the mock upstream with configurable concurrency and streaming.
2. Measure on SQLite and on PostgreSQL: added latency per request at
   p50 and p99, requests per second at saturation, memory at 1 000
   concurrent streams.
3. A 30 minute soak with a mix of requests, logins and tool calls.
   Assert no memory growth beyond a bound and no error.
4. Record method, machine and numbers in `docs/benchmarks.md`.
   Budgets: p50 overhead under 5 ms and p99 under 25 ms on SQLite on
   the reference machine. A miss is reported, and fixed if the cause
   is a defect.

### Accept
- `make bench` runs and prints the table.
- The soak ends with zero errors and stable memory.
- `docs/benchmarks.md` holds the measured numbers, the commit and the
  machine, with no number that was not measured.

### Proof
`make bench`

## T-1004 Journey suite and README proof
- Version: 1.0.0
- Depends: T-1001
- Review: standard
- Type: test
- Spec: SDD.md section "Proving the README", docs/spec/web.md section 6
- Paths: web/e2e/, scripts/quickstart-check.sh, .github/workflows/

### Build
1. One Playwright journey per page listed in the web spec, in both
   themes, at 1440 and 390 pixels wide.
2. Visual snapshots for the dashboard, login and one page per area.
3. `scripts/quickstart-check.sh` runs every command of the README
   Quickstart and of `docs/getting-started.md` as written.
4. All of it runs in CI on Linux. The Rust suite and the quickstart
   check also run on macOS.

### Accept
- `make e2e` covers every page in the web spec (a test compares the
  page list in the spec with the journeys).
- `scripts/quickstart-check.sh` exits 0 on Linux and macOS in CI.
- No accessibility issue of level serious or critical on any page.

### Proof
`make e2e`

## T-1005 Security review and supply chain
- Version: 1.0.0
- Depends: T-1002, T-1004
- Review: security
- Type: chore
- Spec: docs/SPEC.md section 2, factory/SECURITY-CHECKLIST.md
- Paths: factory/reviews/, .github/workflows/, deny.toml, SECURITY.md, docs/

### Build
1. A whole-codebase review: one checklist file per crate under
   `factory/reviews/full/`, each written by a fresh sub-agent given
   that crate, the checklist and the spec. Every finding becomes a fix
   with a test, or a GitHub issue linked in the file.
2. Dependency review: remove unused crates (`cargo machete`), list
   every direct dependency with its purpose in `docs/dependencies.md`,
   confirm `cargo deny` and `npm audit` are clean.
3. Pin every GitHub Action to a commit hash. Least privilege
   `permissions` in every workflow.
4. Release workflow: SBOM, cosign signatures, checksums, build
   provenance attestation.
5. Fill the scope section of `SECURITY.md` from the threat model.

### Accept
- A review file exists for every crate, each ends with `Verdict: pass`.
- No open finding of high severity.
- `actionlint` and a check that every `uses:` is pinned to a hash
  pass.
- `docs/dependencies.md` lists every direct dependency (a test
  compares it with `cargo metadata`).

### Proof
`make gate`

## T-1006 Documentation completion
- Version: 1.0.0
- Depends: T-1004
- Review: standard
- Type: docs
- Spec: SDD.md
- Paths: docs/, README.md, mkdocs.yml

### Build
1. User guide: first login, recovery key, connections, hub keys,
   agents, MCP, skills, chats, workflows, privacy.
2. Operator guide: install per shape, configuration reference
   generated from the config struct, TLS, backup and restore, upgrade,
   monitoring, the recovery runbook for the instance coffer.
3. API reference rendered from `docs/api/openapi.json`.
4. A page on what an administrator can and cannot see, written for
   non-developers.
5. A comparison page that states facts only, each with a link.
6. Every page is in the nav. `docs/SPEC.md` and `docs/spec/*` match
   the code: read each section against the code and fix the page.

### Accept
- `make docs` passes.
- `config_reference_is_current` (generated page drift test).
- No `TODO` left under `docs/` or in `README.md`.

### Proof
`make docs`

## T-1007 Interface freeze
- Version: 1.0.0
- Depends: T-1005, T-1006
- Review: standard
- Type: chore
- Spec: docs/SPEC.md section 4
- Paths: docs/, crates/riehub-server/tests/, crates/riehub-store/tests/snapshots/

### Build
1. Snapshot tests that pin: the OpenAPI document, the config keys, the
   workflow schema, the CLI surface of `riehub` and `rie`, the audit
   event encoding, the vault and record blob formats.
2. `docs/stability.md`: what is stable from 1.0.0, what is not, and
   the deprecation policy (one minor version of warning).
3. Add the 0.9.0 snapshots to the upgrade test.

### Accept
- `interface_snapshots_are_current`.
- `upgrade_from_0_9_0_passes` on all three backends.

### Proof
`cargo test --workspace snapshot`

## T-1008 Release candidate and soak
- Version: 1.0.0
- Depends: T-1003, T-1007
- Review: standard
- Type: chore
- Spec: docs/spec/deploy.md section 2
- Paths: Cargo.toml, web/package.json, CHANGELOG.md, docs/release-checklist.md

### Build
1. Version `1.0.0-rc.1` in `Cargo.toml` and `web/package.json`.
   `CHANGELOG.md` section `## [1.0.0-rc.1] - <today>`.
2. Write `docs/release-checklist.md`: the steps the maintainer runs
   to deploy the candidate at `https://hub.rie.my.id`, and the soak
   criteria: 7 days, zero audit chain breaks, zero canary hits in
   logs, no crash, no high severity issue open.
3. After `finish`, run `git tag -a v1.0.0-rc.1 -m "riehub 1.0.0-rc.1"`
   on `main`. Do not push it. The maintainer deploys and soaks the
   candidate, and the factory stops here until they approve T-1099.

### Accept
- The candidate builds from a clean clone with `make gate` green.
- `docs/release-checklist.md` exists and is in the nav.

### Proof
`make gate`

## T-1099 Release 1.0.0
- Version: 1.0.0
- Depends: T-1008
- Review: standard
- Type: chore
- Gate: human
- Spec: SDD.md section "Releases"
- Paths: Cargo.toml, web/package.json, CHANGELOG.md, README.md

### Build
1. Only after the maintainer confirms the soak passed and runs
   `factory/factory.sh approve T-1099`.
2. Version `1.0.0` in `Cargo.toml` and `web/package.json`.
   `CHANGELOG.md` section `## [1.0.0] - <today>`. README badge.
3. After `finish`, run `factory/factory.sh release 1.0.0`.

### Accept
- `factory/factory.sh release 1.0.0` prints `RELEASED v1.0.0`.

### Proof
`make gate`
