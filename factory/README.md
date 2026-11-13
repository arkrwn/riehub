# Delivery workflow

riehub is delivered as an integrated product. The task cards and
`factory.sh` remain available as planning and status tools. They are
optional. They do not dictate the branch layout, implementation order,
or when a full CI run is required.

## Milestones

| Version | Goal |
|---|---|
| 0.0.1 | Build the full planned feature set into a usable product. |
| 0.0.2 | Validate user journeys, fix defects, and harden security and reliability. |
| 0.1.0 | Release a production-ready version after supported CI checks pass. |

## Delivery loop

1. Choose a useful end-to-end product slice for the current milestone.
2. Implement it across the required layers. Parallelize independent work
   when it reduces elapsed time.
3. Run focused tests and checks while developing.
4. At integration checkpoints, run the CI suite with `make check` or
   the full release gate with `make gate`. Fix failures before calling
   the checkpoint green.
5. Commit and push integrated changes for review. Report what changed,
   what passed, and what remains incomplete.

Use a branch or worktree layout that fits the work. Keep commits small
enough to review. Do not spend time maintaining task bookkeeping when it
does not help product delivery.

## Tools

```bash
factory/factory.sh status  # optional view of legacy task progress
make check                 # build, lint, tests, and project checks
make security              # dependency and supply-chain checks
make docs                  # strict docs build
make gate                  # complete release checkpoint
```

`factory.sh start`, `finish`, `block`, `approve`, and `release` support
the earlier card-based workflow. Use them only when the maintainer
chooses that workflow. In particular, `finish` merges and deletes the
working branch, so do not use it as a routine substitute for pushing a
branch for review.

## Quality bar for 0.1.0

The 0.1.0 release is production-ready. Core setup, login, account
recovery, provider connections, model requests, routing, and the
management UI must work as complete journeys. Cross-user access must be
blocked. Secrets and user content must stay private. Deployment,
backup, restore, upgrades, and operator recovery must be documented.
The supported CI suite must pass on Linux and macOS. Open high-impact
security and reliability defects block the release.

Task cards under `tasks/` and generated progress under `docs/PROGRESS.md`
are historical planning records until the maintainer replaces them with
a new plan. Do not let their old per-task gates delay the active
milestone.
