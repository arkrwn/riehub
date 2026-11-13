# Software design and delivery

Keep the design record useful and the delivery loop short. The product
contract lives in [`docs/SPEC.md`](docs/SPEC.md) and its subsystem pages.
The working rules live in [`AGENTS.md`](AGENTS.md). The repository uses
focused checks while code is changing and the full CI suite at meaningful
integration points.

## Source of truth

- `docs/SPEC.md` and `docs/spec/` define product behavior, security, and
  interfaces. Keep them in agreement with the implementation.
- `docs/adr/` records consequential decisions and their tradeoffs. Routine
  implementation choices do not need an ADR.
- `README.md` describes the current product status and local quickstart.
- [`docs/ROADMAP.md`](docs/ROADMAP.md) describes the three delivery phases.
- Task cards and `factory/factory.sh` are optional planning tools. They do
  not set a required branch layout or per-change approval process.

## Delivery loop

Build integrated product slices and divide independent work across people
or agents when that saves time. Run focused tests while developing. At
integration checkpoints, run CI, fix failures, and push the work for review.
Report the commands that passed and any checks or product behavior that
remain incomplete.

The milestones are:

| Version | Goal |
|---|---|
| `0.0.1` | Build the complete planned product feature set as one integrated system. |
| `0.0.2` | Test the integrated product, fix missing or incorrect behavior, and harden it. |
| `0.1.0` | Ship a production-ready release after the supported CI and end-to-end checks pass. |

## Documentation

The `docs/` directory is built with MkDocs Material. Add new pages to the
`nav` in `mkdocs.yml`. Keep links between pages under `docs/` relative and
use full GitHub URLs for links from `docs/` to repository files outside it.
Use Mermaid blocks for diagrams. Run `make docs` at integration and release
checkpoints.

## Architecture decisions

Add an ADR when a choice changes a security boundary, storage format,
public interface, supported platform, or operating model and has a
meaningful alternative. Keep the context, decision, and main tradeoff
short. ADRs record decisions, they do not prescribe a delivery workflow.

## Quality and security

- Preserve security invariants S1-S12 in [`docs/SPEC.md`](docs/SPEC.md).
- Never commit credentials, private keys, real user data, or infrastructure
  secrets. Use fake test values.
- Keep user content and secrets out of logs and telemetry. Keep server-side
  authorization and tenant isolation in place.
- Keep Windows paths out of the project. Do not add AI attribution to
  commits, code, docs, or product text.
- Do not claim a check passed unless it completed successfully. State when
  a check was not run.

`make check` runs formatting, lint, tests, project rules, and the web
checks. `make security` checks dependencies and supply chain. `make docs`
builds the documentation site. `make gate` runs all three and is the full
release checkpoint. CI runs the supported checks on Linux and macOS.

## Releases

Use Semantic Versioning and keep `CHANGELOG.md` current. The existing
`v0.0.1` tag is the historical foundation baseline. The revised plan uses
the `0.0.1` development phase for complete feature breadth, `0.0.2` for
validation and hardening, and `0.1.0` for the first production-ready
release. Do not move an existing tag to change its meaning.
