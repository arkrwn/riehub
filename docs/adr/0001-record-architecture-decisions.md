# ADR 0001: Record architecture decisions

Status: accepted, updated by ADR 0003

## Decision

- `docs/SPEC.md` and its subsystem pages define current product behavior,
  security requirements, and interfaces.
- An ADR records a consequential decision and its main tradeoff. Routine
  implementation choices do not need an ADR.
- Keep product docs with code changes and update them when behavior or
  security contracts change.

## Consequences

The specification remains the concise source of truth. ADRs preserve the
reasoning behind decisions that would otherwise be easy to revisit.
