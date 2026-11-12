# ADR 0003: A software factory with a gate-owned progress record

Status: accepted

## Context

riehub is built task by task over many sessions, by people and by
coding agents, often small models. Three failure modes cost the most:
an agent declares success without proof, an agent wanders outside its
task, and an agent burns tokens by re-reading and re-running
everything. ADR 0001 says only the
lead marks progress `done`. With agents working unattended, the lead
has to be something that cannot be talked into it.

## Decision

- The plan is a fixed, ordered set of task cards in `factory/tasks/`,
  one file per version from `0.0.1` to `1.0.0`. A card names its
  dependencies, the spec sections to read, the paths it may touch, its
  acceptance tests and a narrow proof command.
- `factory/factory.sh` is the lead. It picks the next card, creates one
  branch per task, runs `make gate`, and only on a green gate for the
  exact committed tree does it record the task as done, merge it and
  delete the branch. Agents never write `factory/state.tsv` or
  `docs/PROGRESS.md`.
- The gate is `make check`, `make security` and `make docs`. It only
  grows. A missing tool fails it.
- Cards marked `Review: security` also need a filled checklist in
  `factory/reviews/`, ideally written by a fresh sub-agent.
- An agent gets three red gates per task. Then it blocks the task with
  a reason and moves to the next runnable one. Nothing loops forever.
- Merging uses pull requests and CI when `gh` and the remote are
  available, and a local squash merge otherwise.
- Tagging `1.0.0`, and the soak before it, are gated on the maintainer.

## Alternatives considered

- **A plan per session, written by the agent:** small models plan
  badly and the plan drifts between sessions.
- **Multi-agent review of every task:** most tasks do not need it, and
  it multiplies cost. Review is reserved for security-relevant cards.
- **Trusting the agent's own report:** the failure this ADR exists to
  remove.

## Proof

`factory/factory.sh` was exercised end to end on a scratch repository:
`next`, `start`, a red gate, a green gate, `finish`, `block`, a human
gate and `release`. The pull request mode needs `gh` and was not run
there.

## Consequences

Changing the plan means editing cards in a pull request, which is
slower than improvising and is the point. The cards must be good: a
wrong card produces wrong work that passes its own tests.
