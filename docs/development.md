# Development

## Repository layout

```
riehub/
  AGENTS.md          # entry point for AI agents (CLAUDE.md -> AGENTS.md)
  SDD.md             # the design and documentation practice this repo follows
  README.md          # what it is, the status, the quickstart, links
  CHANGELOG.md       # user-visible changes per release
  Makefile           # build, fmt, lint, test, rules, check, security, docs, gate
  mkdocs.yml         # docs site config and nav
  deny.toml          # the cargo-deny policy the gate enforces
  factory/           # task cards, the loop driver, the progress record, reviews
  scripts/           # rules-lint.sh, security.sh, install-tools.sh
  docs/              # SPEC.md, spec/, adr/, ROADMAP.md, PROGRESS.md, guides, assets/
  crates/            # the Rust workspace:
    riehub-core      #   ids, errors, config, Secret<T>, Content, clock
    riehub-crypto    #   AEAD, KDFs, hashing, random, constant-time compare
    riehub-store     #   the Store trait and the SQLite backend
    riehub-audit     #   typed, hash-chained audit log
    riehub-server    #   the `riehub` binary: HTTP, CLI, wiring
  .github/           # CI, docs deploy, PR and issue templates
```

The web UI under `web/`, the deployment files under `deploy/` and the
remaining workspace crates are created by their tasks. The crate
boundaries are fixed in the
[specification](SPEC.md#31-crates-and-their-boundaries).

## Building and testing

```bash
make build        # build
make check        # fmt + lint + test + project rules, what CI runs
make security     # cargo deny, and npm audit once web/ exists
make docs         # strict build of the docs site
make gate         # check + security + docs, what a task needs to merge
make tools        # install the tools the gate needs
make docs-serve   # preview the docs site at http://127.0.0.1:8000
```

## The factory

Work is planned as task cards and driven by one script. The full
description is in
[factory/README.md](https://github.com/arkrwn/riehub/blob/main/factory/README.md),
the decision in [ADR 0003](adr/0003-software-factory.md).

```bash
factory/factory.sh status         # progress per version
factory/factory.sh next           # the next task card
factory/factory.sh start T-0001   # one branch per task
# ... build what the card says, test first ...
factory/factory.sh gate           # run the gate on the committed tree
factory/factory.sh finish T-0001  # record proof, merge, delete the branch
```

A person and an AI agent use the same commands. The rules an agent
follows are in
[AGENTS.md](https://github.com/arkrwn/riehub/blob/main/AGENTS.md), the
practice behind them in
[SDD.md](https://github.com/arkrwn/riehub/blob/main/SDD.md).

## Rules that shape the code

- Security invariants S1 to S12 in the [specification](SPEC.md) hold
  in every commit.
- No `unsafe`, no Windows code path, no print macros, no content in
  logs. `scripts/rules-lint.sh` checks the mechanical part.
- Secrets and user content travel in `Secret<T>` and `Content`, which
  cannot be printed or serialised by accident.
- Every route has a class in the route table, and tests are generated
  from the table.
- Tests never call a real provider. They use local mocks and synthetic
  fixtures.

## Contributing

See [CONTRIBUTING.md](https://github.com/arkrwn/riehub/blob/main/CONTRIBUTING.md).
