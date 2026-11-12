# Getting started

riehub has no release yet. This page describes exactly what works at
the current commit, and each release task updates it. Nothing here is
aspirational: every command on this page is run by
`scripts/quickstart-check.sh` once that script exists (version 0.1.0).

## Requirements

- Linux (Ubuntu 24.04 LTS or newer, or equivalent) or macOS. Windows
  is not supported.
- Rust, the exact version in `rust-toolchain.toml`. `rustup` installs
  it on first use.
- `make`, `git`, and Python 3 for the docs site.

## 1. Get the code and check it

```bash
git clone https://github.com/arkrwn/riehub && cd riehub
make check
```

You should see the tests pass and, at the end:

```text
rules-lint: ok
```

## 2. Read the design

```bash
make docs-serve
```

Open <http://127.0.0.1:8000>. Start with the
[overview](overview.md), then the [specification](SPEC.md).

## 3. Work on it

The project is built task by task. See
[development](development.md) for the loop.

## When something goes wrong

| Symptom | Cause | Fix |
|---|---|---|
| `error: toolchain ... is not installed` | the pinned Rust version is missing | `rustup toolchain install` in the repository |
| `rules-lint [print] ...` | a print macro or a forbidden pattern in the code | use `tracing`, or see the header of `scripts/rules-lint.sh` |
| `cargo-deny is not installed` from `make security` | the gate's tools are missing | `make tools` |
| `make docs` fails on a link | a page is missing from the nav or a link is broken | add the page to `mkdocs.yml`, fix the link |

## Clean up

```bash
make clean
rm -rf .venv-docs
```
