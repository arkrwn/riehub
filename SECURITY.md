# Security policy

## Reporting a vulnerability

Please **do not** open a public issue, pull request or discussion for a
security problem.

Report it privately, by either:

- GitHub's private vulnerability reporting: the **Security** tab of this
  repository, then **Report a vulnerability**, or
- email to <hubungi.aja@gmail.com> with the subject `SECURITY: riehub`.

Please include:

- the affected version (tag or commit) and platform,
- what an attacker can do, and what they need first,
- steps or a minimal proof of concept to reproduce it.

## What to expect

- An acknowledgement within 3 working days.
- An assessment and a fix plan within 14 days for confirmed issues.
- Credit in the release notes, unless you ask not to be named.

Please give us a reasonable time to ship a fix before you disclose the
issue publicly.

## Supported versions

| Version | Supported |
|---|---|
| latest release | yes |
| older releases | no, please upgrade |

## Scope

The threat model is in [docs/SPEC.md](docs/SPEC.md), section 2.2. The
security invariants S1 to S12 in section 2 are the promises this
project makes. A way to break one of them is a vulnerability.

Out of scope:

- attacks that already need root on the host, or a debugger attached
  to the running process,
- what an upstream AI provider or MCP server does with the data it is
  sent,
- a deployment that puts a TLS-terminating proxy in front and trusts
  it, including the Cloudflare tunnel used for testing,
- a user's own compromised device or shared hub key.

riehub has no release yet. Until `1.0.0`, only the newest tagged
version is supported.
