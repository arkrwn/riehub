# ADR 0003: Delivery in broad product phases

Status: accepted. Replaces the earlier task-by-task factory workflow.

## Context

Small isolated tasks, per-task gates, and a fixed dependency queue made
integrated product progress too slow. riehub needs visible working
software and a short path from implementation to feedback.

## Decision

- Build the planned product feature set broadly for `0.0.1`.
- Test the integrated product, fix gaps, and pass CI for `0.0.2`.
- Treat `0.1.0` as the first production-ready release.
- Use one loop: build, run the relevant tests and CI, fix failures, and
  push for review.
- Split work across people or agents when ownership and file boundaries
  are clear. Integrate frequently.
- Keep the product and security contracts in `docs/SPEC.md`. Do not
  weaken security requirements to make a build pass.

## Consequences

Feature work can be pushed before the full end-to-end suite exists, but
it must be identified as development work. The integrated product must
pass its tests and CI before `0.0.2` is complete. The `0.1.0` release
is production-ready and ships only after the full acceptance suite and
CI pass.
