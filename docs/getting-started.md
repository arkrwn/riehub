# Getting started

riehub 0.0.1 is the first release. It runs one binary with a health
endpoint, a SQLite store and an append-only, keyed audit chain. This
page describes exactly what works in 0.0.1: build, `riehub init`,
`riehub serve`, `curl /healthz` and `riehub audit verify`. Every
command on this page was run for the release on Linux.

## Requirements

- Linux (Ubuntu 24.04 LTS or newer, or equivalent) or macOS. Windows
  is not supported.
- Rust, the exact version in `rust-toolchain.toml`. `rustup` installs
  it on first use.
- `make`, `git`, and Python 3 for the docs site.

## 1. Get the code and build

```bash
git clone https://github.com/arkrwn/riehub && cd riehub
make build
```

`make build` compiles the workspace. The binary lands at
`target/debug/riehub`:

```bash
target/debug/riehub
```

```text
riehub 0.0.1
```

## 2. Initialise the data directory

`riehub init` reads `./riehub.toml` if it exists, otherwise the
defaults. The defaults put everything under `./data`. Run it in a
scratch directory:

```bash
mkdir riehub-run && cd riehub-run
../target/debug/riehub init
```

```text
riehub init ok
```

The data directory now holds the store:

```text
$ ls -la data/
total 64
drwx------ 2 you you     60 Nov 12 19:27 .
drwxrwxr-x 3 you you     60 Nov 12 19:27 ..
-rw------- 1 you you  65536 Nov 12 19:27 riehub.db
```

The directory is mode `0700` and the database file is mode `0600`.
`riehub init` also creates the audit key file at first server start,
mode `0600`.

## 3. Start the server

```bash
../target/debug/riehub serve
```

The server binds `127.0.0.1:8080` (the dev default) and logs JSON
lines to stdout. Stop it with `Ctrl-C`, and in-flight requests finish
first.

## 4. Health

With the server running, from another terminal:

```bash
curl -si http://127.0.0.1:8080/healthz
```

```text
HTTP/1.1 200 OK
content-type: application/json
strict-transport-security: max-age=31536000; includeSubDomains
x-content-type-options: nosniff
referrer-policy: no-referrer
x-frame-options: DENY
x-request-id: req_01M78KN66R6H9CZGFF81SBVWCV
content-length: 15
date: Thu, 12 Nov 2026 19:27:20 GMT

{"status":"ok"}
```

`/readyz` answers `200` when the store responds and `503` when it
does not.

## 5. Check the audit chain

Every state change writes a typed, HMAC-chained audit event. The
server start is already event 1. From the scratch directory:

```bash
../target/debug/riehub audit verify
```

```text
audit chain ok through seq 1
```

A broken chain exits with code 3 and names the first bad `seq`. Two
more readers exist:

```bash
../target/debug/riehub audit head    # the newest seq and mac
../target/debug/riehub audit export  # one JSON line per event
```

## 6. The config file

One TOML file, `./riehub.toml` by default, with `RIEHUB_*`
environment overrides and profile rules. The full reference is in
[the API spec](api.md#3-configuration). The dev profile binds to
loopback only, while `production` requires TLS or a trusted proxy and
an `https` public URL.

## 7. Work on it

The project is built task by task. See
[development](development.md) for the loop.

## When something goes wrong

| Symptom | Cause | Fix |
|---|---|---|
| `error: toolchain ... is not installed` | the pinned Rust version is missing | `rustup toolchain install` in the repository |
| `rules-lint [print] ...` | a print macro or a forbidden pattern in the code | use `tracing`, or see the header of `scripts/rules-lint.sh` |
| `cargo-deny is not installed` from `make security` | the gate's tools are missing | `make tools` |
| `make docs` fails on a link | a page is missing from the nav or a link is broken | add the page to `mkdocs.yml`, fix the link |
| `invalid: server.bind: the dev profile binds to loopback only` | a dev config that binds elsewhere | use `127.0.0.1`, `localhost` or `::1`, or switch to `production` |
| `invalid: database file: ...` | the data directory is missing or its mode is wrong | run `riehub init`, keep the directory mode at `0700` |

## Clean up

```bash
make clean
rm -rf .venv-docs
```
