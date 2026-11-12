# The riehub factory

This directory turns the plan into merged, proven work, one task at a
time. Agents follow [`AGENTS.md`](../AGENTS.md). This page explains the
machine to people. The decision behind it is ADR 0003.

## Parts

| Part | Role |
|---|---|
| `tasks/M00-*.md` to `tasks/M10-*.md` | The task cards. One file per version, cards in build order. The cards are the plan. |
| `factory.sh` | The driver. It picks the next card, creates the branch, runs the gate, records proof, merges and deletes the branch. |
| `state.tsv` | The record of finished tasks. Only `factory.sh finish` writes it. |
| `SECURITY-CHECKLIST.md` | The review template for cards marked `Review: security`. |
| `reviews/T-NNNN.md` | One filled checklist per security-reviewed task. |

`docs/PROGRESS.md` is generated from the cards and `state.tsv`.

## A task card

```
## T-0101 Per-user vault on cofferak-core
- Version: 0.1.0            the release the task belongs to
- Depends: T-0005, T-0006   tasks that must be done first, or `none`
- Review: security          `security` or `standard`
- Type: feat                commit type, default `feat`
- Gate: human               only on tasks the maintainer must open
- Spec: ...                 the only pages the agent needs to read
- Paths: ...                the only places the agent may change

### Build      numbered steps
### Accept     one line per test that must exist and pass
### Proof      the narrow command to run while working
```

## The gate

`make gate` runs, in order:

1. `make check`: format, lint with warnings as errors, tests, and
   `scripts/rules-lint.sh` (mechanical checks for the project rules).
2. `make security`: `cargo deny check`, and `npm audit` once `web/`
   exists. A missing tool fails the gate.
3. `make docs`: strict build of the docs site.

Later tasks extend `check` with the web build, the end-to-end suite and
the privacy canary test. The gate only ever grows.

`factory.sh gate` runs it on the committed tree and stores the tree
hash. `factory.sh finish` refuses a tree that has no green gate, a
branch with AI attribution in a commit, a `feat` or `fix` task without
a changelog line, and a security task without a passed review file.

## Merge modes

`factory.sh` picks the mode itself. `FACTORY_MODE=pr` or
`FACTORY_MODE=local` forces one.

- **pr**: used when `gh` is installed and logged in and `origin` is
  reachable. Push the branch, open a pull request, wait for CI, squash
  merge, delete the branch on both sides. CI also runs the gate on
  macOS, which the local gate on a Linux machine cannot.
- **local**: used otherwise. Squash merge into the local `main`, delete
  the branch. Nothing is pushed. The maintainer pushes `main` later.

## States

`todo`, `doing`, `blocked`, `done`. Only `done` is committed. `doing`
and `blocked` live in `.git/factory/local.tsv`, so they belong to one
working clone and never cause merge noise.

A task is blocked after three red gates, or when it would break a rule.
`factory.sh block` keeps the branch with the work in progress, records
the reason and moves on to the next task whose dependencies are done.
`factory.sh start <ID>` on a blocked task resumes its branch.

## For the maintainer

```bash
factory/factory.sh status          # progress, blocked tasks with reasons
factory/factory.sh approve T-1099  # open a human-gated task
factory/factory.sh release 0.1.0   # run by the release card, tags v0.1.0
```

To change the plan, edit the cards in a normal pull request. Never
renumber a card that is already done. Add a new card with a new number.
