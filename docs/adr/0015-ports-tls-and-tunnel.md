# ADR 0015: Port 8080 for development, 443 with native TLS for production

Status: accepted

## Context

The instance is tested at `https://hub.rie.my.id` through a Cloudflare
tunnel, and must later serve production traffic on 443. A proxy that
terminates TLS can read every prompt, which matters for a system whose
main promise is that operators cannot.

## Decision

- Profile `dev`: plain HTTP on `127.0.0.1:8080`, loopback only. A
  tunnel or local proxy may sit in front for testing.
- Profile `production`: riehub terminates TLS 1.3 itself on 443 (8443
  inside containers, published as 443). Certificates are files and are
  reloaded on `SIGHUP`. riehub does not include an ACME client.
- A TLS-terminating proxy in production is allowed only when listed in
  `server.trusted_proxies`. The docs say plainly that such a proxy can
  read traffic.
- The client IP header is honoured only from trusted proxies.

## Alternatives considered

- **Always behind a reverse proxy:** common, and it puts a second
  party in the plaintext path by default.
- **Built-in ACME:** convenient, more code that talks to the internet
  with a private key nearby. cert-manager and standalone ACME clients
  already do this.

## Consequences

Operators provide certificates. Testing through the tunnel is not
representative of the production trust model, and test accounts should
be used there.
