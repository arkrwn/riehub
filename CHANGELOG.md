# Changelog

All notable changes to riehub are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Configuration loading in `riehub-core` and the binary: the spec TOML
  with defaults, `RIEHUB_*` env overrides, profile validation, plus
  `riehub serve --config` and `riehub doctor`.
- Core types in `riehub-core`: typed ids (`<prefix>_<ULID>`), `Secret<T>`
  and `Content` that never leak through `Debug`, `Display` or `Serialize`,
  the eight-class `Error`, `Clock` with a test double, and `Owner`.
- Project skeleton: SDD practice, AGENTS.md, ADR log, spec and progress
  table, MkDocs Material documentation site with GitHub Pages deploy, CI.
- The design: specification with security invariants S1 to S12, ten
  subsystem pages, ADRs 0002 to 0015, and the roadmap to 1.0.0.
- The factory: task cards for versions 0.0.1 to 1.0.0, the
  `factory/factory.sh` driver, the gate (`make gate`) and the project
  rules lint.

[Unreleased]: https://github.com/arkrwn/riehub/commits/main
