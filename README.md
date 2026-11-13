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
  <a href="CHANGELOG.md"><img alt="Version" src="https://img.shields.io/badge/version-0.0.1-14532D.svg"></a>
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

**riehub** is being built as a self-hosted server that gives every
person in a team one private entry point to AI. The target product
routes agents and SDKs to model providers, and holds each user's own
provider accounts, MCP connectors, skills, chats and workflows. The
The intended security model keeps users' content private from other
users and operators. This is the project goal, not a claim that the
current release already provides every capability.

## Product goals

- **One router, every format.** The target API will support OpenAI Chat
  Completions and Responses, Anthropic Messages and Gemini in front of providers,
  with routing groups, failover and budgets.
  [Router spec](docs/spec/router.md)
- **Each user holds their own keys.** The design calls for credentials
  and content to be encrypted in a per-user vault under keys that only
  a secret the user presents can open. The vault is built on
  [cofferak](https://github.com/arkrwn/cofferak).
  [Security spec](docs/spec/security.md)
- **Operators see metadata, never content.** The target audit log will
  record who did what without prompts or secrets.
  [Privacy model](docs/adr/0004-privacy-model.md)
- **Your tools in one place.** The target product will provide one MCP
  endpoint per user in front of their connectors, plus private skills
  and YAML workflows.
  [MCP](docs/spec/mcp.md) · [Automation](docs/spec/automation.md)

## Status

The existing `v0.0.1` tag is an early foundation baseline with the
server, SQLite store, audit chain and health endpoints. It is not a
complete product or suitable for production. The new delivery phases
build the full planned product as the `0.0.1` development milestone,
then publish fast, testable `0.0.2` through `0.0.100` previews as
feedback and fixes arrive. Make an integrated `0.1.0` candidate available
as soon as possible; call it production-ready only when its release
criteria pass.
The existing tag stays as historical baseline. The
[progress page](docs/PROGRESS.md) records the earlier task plan and may
not reflect the revised milestones.

## Quickstart

```bash
git clone https://github.com/arkrwn/riehub && cd riehub
make build        # compiles the binary to target/debug/riehub
cd /tmp && mkdir riehub-run && cd riehub-run
/path/to/riehub/target/debug/riehub init    # data dir mode 0700
/path/to/riehub/target/debug/riehub serve   # binds 127.0.0.1:8080
```

Then, from another terminal in the same directory:

```bash
curl -si http://127.0.0.1:8080/healthz     # {"status":"ok"}
/path/to/riehub/target/debug/riehub audit verify
```

The full walkthrough, with real output, is in
[getting started](docs/getting-started.md).

You need Rust (the version in `rust-toolchain.toml`), Node.js 22 or
newer, `npm`, `make`, `git` and Python 3 for the docs site, on Linux or
macOS. The Quickstart is for local development. Production deployment
guidance will be published with the `0.1.0` release.

## Documentation

| | |
|---|---|
| [Overview](docs/overview.md) | What riehub is, why, and how it works |
| [Spec](docs/SPEC.md) and [ADRs](docs/adr/README.md) | The contract, the security invariants and the design decisions |
| [Roadmap](docs/ROADMAP.md) | Versions `0.0.1` to `1.0.0` |
| [Progress](docs/PROGRESS.md) | Status generated from the earlier task plan |
| [Development](docs/development.md) | Repository layout, building, testing, the factory |

The same pages are built into a documentation site with MkDocs Material
(see [`mkdocs.yml`](mkdocs.yml)) and published to GitHub Pages.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Contributions require accepting
the project's CLA ([CLA.md](CLA.md)) on your first pull request. Read
[AGENTS.md](AGENTS.md) for the current build, test and review workflow.

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
