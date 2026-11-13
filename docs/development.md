# Development

riehub is built in three product milestones:

| Version | Goal |
|---|---|
| `0.0.1` | Build and integrate the planned feature set as one usable product. Use focused checks while implementing and record known gaps. |
| `0.0.2` | Run end-to-end journeys and operational checks, fix gaps, and harden security and reliability. |
| `0.1.0` | Ship a production-ready product after supported CI checks and release requirements pass. |

The current `v0.0.1` tag is an early baseline. It does not mean the
feature-complete milestone is done. See the [roadmap](ROADMAP.md) for
the release goals.

## Repository layout

```
riehub/
  AGENTS.md          # concise guidance for people and coding agents
  README.md          # product overview, status and quickstart
  CHANGELOG.md       # user-visible changes
  Makefile           # build, checks, security and docs commands
  docs/              # product spec, subsystem specs, ADRs and guides
  factory/           # optional task cards and legacy progress tools
  scripts/           # rules lint, dependency checks and tool setup
  crates/            # Rust workspace and server
  web/               # React and TypeScript UI
  deploy/            # deployment assets as they are implemented
```

The product contract is in [SPEC.md](SPEC.md). Its subsystem pages
describe identity, security, routing, storage, MCP, automation, the web
UI and deployment.

## Build and checks

```bash
make build        # build the Rust workspace and web UI
make fmt          # check Rust formatting
make lint         # run Rust lint checks
make test         # run Rust tests
make check        # CI checks, including web lint, tests and build
make security     # dependency and supply-chain checks
make docs         # strict documentation build
make gate         # full release checkpoint
make tools        # install local gate tools
make docs-serve   # preview docs at http://127.0.0.1:8000
```

Use focused commands during implementation. Run `make check` at
integration checkpoints and before pushing a release candidate. Fix
failures before reporting a checkpoint as green. If a check cannot run,
say which check and why.

## Delivery workflow

Build broad, integrated product slices. Split work across people or
agents when the tasks have clear, independent ownership. Integrate
frequently and push working changes for review. Do not delay useful
product progress to complete small cards one by one.

The factory task cards and commands are optional planning tools. They
remain available through
[`factory/README.md`](https://github.com/arkrwn/riehub/blob/main/factory/README.md).
They do not require test-first development, one branch per card, or an
automatic merge. `factory/factory.sh status` can still show the earlier
task plan.

## Security and correctness

- Preserve security invariants S1-S12 in the [specification](SPEC.md).
- Do not commit credentials, keys, user data or private infrastructure
  details. Use clearly fake test values.
- Keep credentials and user content out of logs, errors, metrics,
  traces, audit metadata and browser storage. Theme preference is the
  only browser preference stored in local storage.
- Enforce authorization on the server and treat client input as
  untrusted. Route user-controlled outbound requests through the
  guarded egress client.
- Do not add `unsafe` or Windows code paths.
- Tests must use local mocks and synthetic fixtures rather than real
  providers.

## Contributing

See [CONTRIBUTING.md](https://github.com/arkrwn/riehub/blob/main/CONTRIBUTING.md).
