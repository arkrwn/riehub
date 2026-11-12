<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/assets/logo-dark.svg">
    <img src="docs/assets/logo.svg" alt="riehub" width="260">
  </picture>
</p>

<p align="center">
  <strong>One private hub for every AI model, tool, skill and workflow.</strong><br>
  multi-user · encrypted per user · one Rust binary · self-hosted<br>
  Linux and macOS. No cloud account, no telemetry, no Windows.
</p>

<p align="center">
  <a href="https://github.com/arkrwn/riehub/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/arkrwn/riehub/actions/workflows/ci.yml/badge.svg?branch=main"></a>
  <a href="https://github.com/arkrwn/riehub/actions/workflows/docs.yml"><img alt="Docs" src="https://github.com/arkrwn/riehub/actions/workflows/docs.yml/badge.svg?branch=main"></a>
  <a href="CHANGELOG.md"><img alt="Version" src="https://img.shields.io/badge/version-unreleased-14532D.svg"></a>
  <a href="LICENSE"><img alt="License: Apache-2.0" src="https://img.shields.io/badge/license-Apache--2.0-blue.svg"></a>
  <a href="docs/adr/0002-rust-single-binary-linux-macos.md"><img alt="Rust, no unsafe" src="https://img.shields.io/badge/rust-no%20unsafe-orange.svg"></a>
</p>

<p align="center">
  <a href="#status">Status</a> ·
  <a href="docs/overview.md">How it works</a> ·
  <a href="docs/SPEC.md">Spec</a> ·
  <a href="docs/ROADMAP.md">Roadmap</a> ·
  <a href="docs/adr/README.md">Decisions</a> ·
  <a href="https://arkrwn.github.io/riehub/">Docs site</a>
</p>

---

**riehub** is a self-hosted server that gives every person in a team
one private entry point to AI. It routes any agent or SDK to any model
provider, and it holds each user's own provider accounts, MCP
connectors, skills, chats and workflows. Many users share one
instance. Nobody, including the administrator, can read what another
user sends through it.

- **One router, every format.** OpenAI Chat Completions, OpenAI
  Responses, Anthropic Messages and Gemini in front of any provider,
  with routing groups, failover and budgets.
  [Router spec](docs/spec/router.md)
- **Each user holds their own keys.** Credentials and content are
  encrypted under keys that only a secret the user presents can open,
  in a per-user vault built on
  [cofferak](https://github.com/arkrwn/cofferak).
  [Security spec](docs/spec/security.md)
- **Operators see metadata, never content.** A hash-chained audit log
  records who did what, without prompts or secrets.
  [Privacy model](docs/adr/0004-privacy-model.md)
- **Your tools in one place.** One MCP endpoint per user in front of
  all their connectors, plus private skills and YAML workflows.
  [MCP](docs/spec/mcp.md) · [Automation](docs/spec/automation.md)

## Status

riehub is being built in the open, version by version. Nothing is
released yet. The [roadmap](docs/ROADMAP.md) lists what each version
from `0.0.1` to `1.0.0` contains, and the
[progress page](docs/PROGRESS.md) shows what is done, with proof.

This README gains a Quickstart with the first release that has
something to run. Until then:

```bash
git clone https://github.com/arkrwn/riehub && cd riehub
make check        # format, lint, tests, project rules
make docs-serve   # read the design at http://127.0.0.1:8000
```

You need Rust (the version in `rust-toolchain.toml`), `make`, `git`
and Python 3 for the docs site, on Linux or macOS.

## Documentation

| | |
|---|---|
| [Overview](docs/overview.md) | What riehub is, why, and how it works |
| [Spec](docs/SPEC.md) and [ADRs](docs/adr/README.md) | The contract, the security invariants and the design decisions |
| [Roadmap](docs/ROADMAP.md) | Versions `0.0.1` to `1.0.0` |
| [Progress](docs/PROGRESS.md) | What is done, with proof |
| [Development](docs/development.md) | Repository layout, building, testing, the factory |

The same pages are built into a documentation site with MkDocs Material
(see [`mkdocs.yml`](mkdocs.yml)) and published to GitHub Pages.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Contributions require accepting
the project's CLA ([CLA.md](CLA.md)) on your first pull request. People
and AI agents both start with [AGENTS.md](AGENTS.md) and [SDD.md](SDD.md).
The work is planned as task cards in [`factory/`](factory/README.md).

## Security

Security is the first requirement of this project, ahead of features.
Please report vulnerabilities privately, see [SECURITY.md](SECURITY.md).
The threat model, including what riehub does not protect against, is in
[docs/SPEC.md](docs/SPEC.md).

## License

Apache License 2.0, see [LICENSE](LICENSE) and [NOTICE](NOTICE). The
`riehub` name and logo are governed separately by the
[trademark policy](TRADEMARK.md): the license covers the code, not the
name.
