# AGENTS.md

Working guidance for people and coding agents in riehub. `CLAUDE.md`
links here. Keep this guide short and use the product specs for behavior.

## Product and delivery

riehub is a self-hosted, multi-user AI hub. The server is Rust. The web
UI is TypeScript. Supported platforms are Linux and macOS.

Deliver in three milestones:

1. **0.0.1: feature-complete build.** Implement the planned user-facing
   product end to end. Optimize for a complete, integrated product, not
   for finishing many small cards or polishing isolated modules.
2. **0.0.2: validation and hardening.** Exercise real user journeys,
   fix gaps and defects, improve security and reliability, and keep the
   CI suite green.
3. **0.1.0: production-ready release.** Ship only when the core
   journeys work, security and privacy requirements hold, deployment
   and recovery are documented, and CI passes on supported platforms.

Use the simplest useful loop:

1. Build a complete slice that advances the current milestone.
2. Run focused tests while developing. Run the full CI checks at
   integration and release checkpoints.
3. Fix failures. Report any check that could not be run.
4. Commit and push the integrated work for review.

Use judgment about the size of a slice and the timing of checks. Task
cards and the factory are planning aids. They do not require one task
per branch, test-first development, or a separate full gate after every
small change. Do not mark an unreleased milestone complete while core
features or required checks are missing.

## Security and correctness

- Never commit secrets, credentials, private keys, databases, personal
  infrastructure details, or real user data. Use clearly fake test
  values.
- Preserve the security invariants S1-S12 in `docs/SPEC.md`. If a
  proposed change would violate one, explain the tradeoff and choose a
  safe design before proceeding.
- Keep credentials and user content out of logs, errors, metrics,
  traces, audit metadata, and browser storage. Theme preference may be
  stored in browser local storage. Authentication uses the HttpOnly
  cookie.
- Keep authorization on the server. Treat all client input as
  untrusted. Route user-controlled outbound requests through the
  guarded egress client.
- Do not claim a command passed unless it completed with exit code 0.
  Name checks that were not run.
- Do not add an AI signature, co-author trailer, or tool attribution to
  commits, source, docs, reviews, or product text. Use the configured
  maintainer identity for commits.
- Keep Windows code paths out of the project.

## Collaboration

Use parallel agents when work can be split across independent modules,
files, or review tasks. Give each agent a clear scope and avoid assigning
conflicting edits to the same files. Integrate and review the results
before committing. Use a separate worktree when concurrent code edits
would otherwise share files. Do not delegate changes that expose
secrets or user data.

Preserve existing user changes. Avoid force pushes and destructive
commands. Follow explicit user direction about scope, branches, and
pushes.

## Specs and decisions

Product behavior and security requirements live in `docs/SPEC.md` and
its subsystem pages. Keep them concise and actionable. ADRs record only
important decisions with lasting tradeoffs. Do not write an ADR for an
ordinary implementation choice. When code changes behavior, update the
relevant spec and user-facing docs.

## Useful commands

| Need | Command |
|---|---|
| Build | `make build` |
| Focused or full tests | `make test` or the relevant package test |
| Lint and formatting | `make lint` and `make fmt` |
| CI checks | `make check` |
| Dependencies and supply chain | `make security` |
| Docs | `make docs` |
| Full release checkpoint | `make gate` |

CI runs the gate. Use focused commands during development and the full
checks at meaningful integration points.
