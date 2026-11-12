# ADR 0014: Plugins are WebAssembly with explicit capabilities

Status: accepted

## Context

Both reference projects let operators add middleware: one runs
JavaScript packages, the other native Go or WASM. A plugin in the
request path sees prompts. Native or scripted plugins would also get
the process's memory, files and network.

## Decision

- Plugins are WebAssembly components run with `wasmtime` (version
  0.9.0). Hooks: `on_request`, `on_event`, `on_response`, each over the
  canonical model.
- A plugin has no file system, no clock beyond a coarse one, and no
  network unless its manifest asks for named hosts and the installer
  approves them. Network access goes through the guarded client.
- Fuel and memory limits bound every call.
- An admin installs a plugin for the instance. A user enables it for
  their own traffic. A plugin never runs on a user's traffic unless
  that user turned it on, except guardrail plugins required by an
  access profile, which are shown to the user.
- A plugin error refuses the request unless the plugin was enabled
  with `on_error = "skip"`.

## Alternatives considered

- **Native dynamic libraries or scripts:** full process access.
- **No plugins:** forces forks for small customisations.

## Consequences

Plugin authors compile to WASM. Plugins cannot do everything a native
one could, on purpose.
