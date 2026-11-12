# Security review: T-NNNN

Copy this file to `factory/reviews/T-NNNN.md`. Answer every item with
evidence: `path:line` or a test name. Write `n/a` and one reason when
an item does not apply to this change. Tick a box only when the
evidence is real. The invariants S1 to S12 are in `docs/SPEC.md`,
section 2.

Diff reviewed: `git diff main...HEAD` at commit `<short hash>`

## Secrets and content

- [ ] S1. Every new stored field that holds a credential or user content is encrypted under a user-derived key (or the instance seal for instance secrets). Evidence:
- [ ] S1. No new plaintext column, file, cache or temp file can hold a credential or user content. Evidence:
- [ ] S2. No log line, error message, metric label, trace attribute or audit field can carry a credential or user content. New types that hold them use `Secret<T>` or `Content`. Evidence:
- [ ] S9. Key material and plaintext secrets are zeroized on drop and never cloned into long-lived structures. Evidence:

## Access

- [ ] S3. Every new route has a class in the route table (`public`, `self`, `admin_meta`, `key`) and a test for that class. No route returns another user's content or secrets to any role. Evidence:
- [ ] S4. Nothing new lets one principal obtain a session, hub key or unlock slot for another user. Evidence:
- [ ] S5. Every new storage access for user-owned data takes a `UserScope`. There is a cross-user test that gets "not found". Evidence:
- [ ] Authorization is checked on the server for every new action, not only hidden in the UI. Evidence:

## Failure and input

- [ ] S6. Each new check (auth, limit, budget, masking, guardrail, audit write) fails closed, and there is a test for the failure path. Evidence:
- [ ] S7. Each new state change and each routed request writes an audit event with allowlisted fields only. Evidence:
- [ ] All new external input has size limits and is parsed with a typed parser. No string-built SQL, shell command or path. Evidence:
- [ ] S11. Every new outbound URL that a user can influence goes through the guarded HTTP client (SSRF rules). Evidence:
- [ ] Comparisons of secrets and tags are constant time. Evidence:
- [ ] No new `unwrap`, `expect`, index or arithmetic can panic on external input. Evidence:

## Transport and platform

- [ ] S8. New cookies, headers and TLS settings follow the spec. Evidence:
- [ ] S10. New dependencies are named by the card or the spec, are maintained, and pass `cargo deny check` or `npm audit`. List them:
- [ ] S12. No telemetry, no third-party origin, no call home. Evidence:
- [ ] No `unsafe`, no Windows code path. Evidence:
- [ ] R1. No AI signature or mark in the diff or in any commit of the branch, and no co-author trailer. Evidence:

## Tests

- [ ] Each `Accept` line of the card maps to a test that fails without the change. List test names:
- [ ] The privacy canary test still passes and covers the new surface, if the task adds one. Evidence:

## Findings

List anything you had to fix during the review, and anything left open
with the issue it was filed under. Write `none` if there is nothing.

Verdict: fail
