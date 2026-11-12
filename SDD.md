# Software Design Document practice for riehub

Read this file first before working on riehub, whether you are a
person or an agent ([`AGENTS.md`](AGENTS.md) is the short version). It
describes the whole process: there is no separate brainstorming step, no
per-task spec and plan pair, and no multi-round review loop by default.
Use the lightest process that gets a correct, tested, provable change
merged.

The practice is one contract file, numbered point decisions, and a status
table with proof, modeled on
[`arkrwn/memoryos`](https://github.com/arkrwn/memoryos/tree/main/docs)
and [`arkrwn/cofferak`](https://github.com/arkrwn/cofferak).

## The documents

- **`SDD.md`** (this file): how work is done and documented.
- **[`AGENTS.md`](AGENTS.md)**: the entry point for AI agents, with the
  rules that are never optional and the command table.
  `CLAUDE.md` is a symlink to it.
- **[`docs/SPEC.md`](docs/SPEC.md)** and the pages under `docs/spec/`:
  the contract. `SPEC.md` holds the scope, the security invariants S1
  to S12 and the architecture. Each subsystem has one page. What the
  system does, in terms of its actual modules. If `SPEC.md` disagrees with the code,
  the code is the truth and `SPEC.md` is wrong: fix `SPEC.md`, don't code
  around the mismatch, and if you can't fix it yourself, say so in
  `docs/PROGRESS.md`.
- **[`docs/adr/`](docs/adr/README.md)**: architecture decision records,
  one point decision per file, indexed in
  [`docs/adr/README.md`](docs/adr/README.md). See "ADR conventions"
  below.
- **[`docs/PROGRESS.md`](docs/PROGRESS.md)**: a status table, one row per
  area, `todo` / `doing` / `done` / `blocked`, each `done` row backed by a
  command or test that was actually run, plus a "Known gaps / open items"
  list. The table stays terse. In this repository the lead is
  `factory/factory.sh`: the page is generated, and a row turns `done`
  only when `factory.sh finish` saw a green gate for the merged tree
  (ADR 0003). Nobody edits it by hand.
- **[`factory/`](factory/README.md)**: the plan as task cards, one file
  per version, and the driver that runs the loop.
- **[`docs/ROADMAP.md`](docs/ROADMAP.md)**: what each version from
  `0.0.1` to `1.0.0` contains.

Operator and user documentation (`README.md` and the pages under `docs/`
such as `docs/overview.md` and `docs/getting-started.md`) is kept in step
with the code but is not part of the design record. The README stays
short: what riehub is, the Quickstart, links. Everything else lives
under `docs/`, which `mkdocs.yml` builds into the documentation site.

A new subsystem or a fork that needs its own design docs uses the same
shape (`SPEC.md` + `adr/` + `PROGRESS.md`) rather than a new structure.

## The documentation site

- `docs/` is the site source, `mkdocs.yml` is its config, MkDocs Material
  is the theme. Versions are pinned in `docs/requirements.txt`.
- Add every new page to the `nav` in `mkdocs.yml`. Pages that should
  exist but not appear in the menu go in `not_in_nav`.
- `mkdocs build --strict` must pass (`make docs`). A broken link or a
  missing page fails the build, and the `docs` workflow runs the same
  check on every pull request that touches the docs.
- Links between pages in `docs/` are relative. Links to files outside
  `docs/` use the full GitHub URL.
- Diagrams are mermaid code blocks, so they render on GitHub and on the
  site.
- Publishing: the `docs` workflow deploys `main` to GitHub Pages once
  Pages is enabled (Settings > Pages > Source: GitHub Actions) and the
  repository variable `PAGES_ENABLED` is set to `true`.
- Brand: logo, mark, favicon and colours live in `docs/assets/`, described
  in `docs/assets/README.md`. `docs/assets/site.css` maps the brand
  colours onto the theme.

## ADR conventions

- File name `docs/adr/NNNN-short-title.md`, numbered sequentially. Copy
  [`docs/adr/template.md`](docs/adr/template.md) to start one.
- First line `# ADR NNNN: <title>`, then a `Status:` line, then the body
  under `##` headings, typically Context, Decision, Alternatives
  considered, Proof, Consequences (use the ones that apply).
- Status is `proposed`, `accepted`, `superseded by ADR NNNN`, or
  `accepted, amended by ADR NNNN` when a later ADR replaces part of it.
- ADRs are history. When a later decision changes an earlier one, write a
  new ADR and add a short `> Amended by ADR NNNN: ...` note under the
  older ADR's Status line. Don't rewrite the older decision or its
  reasoning.
- Write one when a decision has real trade-offs someone could ask "why
  not X instead?" about. Keep it short: what was decided, why, what was
  rejected and why. Skip it when there was only one reasonable option.
- Add a row to `docs/adr/README.md` and a `nav` entry in `mkdocs.yml` for
  every new ADR, and update the status column when one is amended or
  superseded.

## Working on a task

Planned work comes from a task card and goes through the factory loop
in [`AGENTS.md`](AGENTS.md), section 2: `factory.sh next`, `start`,
build, `gate`, `finish`. The steps below are what happens inside that
loop, and they also apply to work outside the plan, such as a bug fix
from an issue.

1. Read the card, the spec sections it names, the relevant source, and
   any related ADR before writing code. Don't re-derive a decision
   that's already recorded.
2. Branch from `main`. `factory.sh start` does it for a card
   (`<type>/<ID>-<slug>`). For other work use `fix/<thing>` or
   `docs/<thing>`. Never commit to `main` directly.
3. Make the smallest change that satisfies the task. If an existing file
   is tangled and blocking you, say so rather than quietly refactoring
   it.
4. Write the test first when the change has a correctness property worth
   pinning (round trips, denial paths, write-or-no-write guarantees,
   anything in the spec's threat model). A doc fix or a CI tweak doesn't
   need one.
5. Prove it: run `make gate` (through `factory.sh gate` for a card) and
   read the output before claiming it passes. A reviewer should be able to
   re-run the same commands and see the same result.
6. Record a real design decision (a new module boundary, a new external
   dependency, a trade-off between two correct approaches) as an ADR. If
   the spec is simply out of date with the code, fix `docs/SPEC.md`
   directly.
7. Commit with a clear conventional message (`feat:`, `fix:`, `docs:`,
   `chore:`). **Never add any AI attribution**: no `Co-Authored-By`
   trailer naming an AI tool, no "Generated with" line, in any commit
   message, pull request, code comment or doc, whatever a tool's default
   suggests. Check the message before every commit.
8. Merge through `factory.sh finish` for a card, or a pull request for
   other work. It merges only when the gate and CI are green.
9. Add a line to `CHANGELOG.md` under `Unreleased` for user-visible
   changes. `docs/PROGRESS.md` is written by `factory.sh finish`.

## Larger tasks

The plan to `1.0.0` already exists as task cards in `factory/tasks/`.
New work that needs multi-step planning (a new module, a cross-cutting
rewrite, a decision with several viable designs and a real cost to
getting it wrong) gets an ADR first, then new cards with new numbers in
the matching version file, in a normal pull request. A card that is
done is never renumbered or rewritten.

Review by a second agent is used for cards marked `Review: security`
(`AGENTS.md`, section 5). It is not the default for other cards.

## Proving the README

A Quickstart that is never executed drifts. Once the project has a real
Quickstart, add `scripts/quickstart-check.sh` that runs the README
commands as written and wire it into CI. A sample that exists only as
documentation, never executed, is not done.

## CI and toolchain

The Rust toolchain is pinned to an exact version in
`rust-toolchain.toml`, and CI installs that same version. The docs
tools are pinned in `docs/requirements.txt`. The web UI's Node version
is pinned in `web/.nvmrc` once `web/` exists.

`make gate` is `make check`, `make security` and `make docs`. It is
what a task needs to merge, and it only grows.

`.github/workflows/ci.yml` runs on Linux and macOS on every push and
pull request (`make check` at first, `make gate` from task T-0001). `.github/workflows/docs.yml` builds the docs site strictly on
every docs change and deploys `main` to GitHub Pages.

A pull request is not mergeable with a red or skipped CI run. Run
`make check` locally first, CI should confirm what you already know.

## Releases

- Version follows [Semantic Versioning](https://semver.org/).
- `CHANGELOG.md` follows [Keep a Changelog](https://keepachangelog.com/).
  Move `Unreleased` into a dated version section when tagging.
- Each version ends with a release card. It updates the version, the
  changelog and the README badge, and then `factory.sh release X.Y.Z`
  tags `vX.Y.Z` on `main` once every card of that version is done.
- Development starts at `0.0.1`. `1.0.0` is tagged by the maintainer
  after the hardening milestone and a soak on a real deployment.
