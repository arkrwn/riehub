# ADR 0001: Record architecture decisions

Status: accepted, amended by ADR 0003

> Amended by ADR 0003: the progress table is written by `factory/factory.sh` on a green gate, not by hand.

## Context

Design choices get lost in chat threads and commit messages, and people
and AI agents alike re-argue decisions that were already settled. The
project needs one place that says what was decided and why, and one
place that says what the system does.

## Decision

riehub follows the practice in
[`SDD.md`](https://github.com/arkrwn/riehub/blob/main/SDD.md):

- [`docs/SPEC.md`](../SPEC.md) is the contract. The code wins a
  disagreement, and the spec gets fixed.
- `docs/adr/NNNN-short-title.md` records one point decision per file,
  indexed in [`docs/adr/README.md`](README.md).
- [`docs/PROGRESS.md`](../PROGRESS.md) tracks each area as `todo`,
  `doing`, `done` or `blocked`, and every `done` row names the command or
  test that proves it.
- [`AGENTS.md`](https://github.com/arkrwn/riehub/blob/main/AGENTS.md)
  is the entry point for AI agents and points back to all of the above.

## Alternatives considered

- **A wiki or an external doc tool:** drifts from the code because it is
  not reviewed in the same pull request.
- **A full spec and plan per task:** too heavy for most changes. A plan
  is written only for large, multi-session work (`docs/PLAN.md`).

## Consequences

Every real trade-off costs a short file. In return, the reasoning is
versioned with the code, reviewed with it, and published on the docs
site.
