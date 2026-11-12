# Deployment

How riehub runs. Everything here ships in `deploy/` from version 0.8.0.
Decisions: ADR 0002 (platforms), ADR 0012 (scaling), ADR 0015 (ports
and TLS).

## 1. Shapes

| Shape | Store | Nodes | Seal |
|---|---|---|---|
| Standalone on a Linux VM (Ubuntu 24.04 LTS or newer) | SQLite | 1 | `systemd` |
| Standalone on macOS | SQLite | 1 | `keychain` |
| Docker or Compose | SQLite on a volume, or PostgreSQL | 1 or more | `file` |
| Kubernetes (Helm chart) | PostgreSQL or MongoDB | 2 or more | `file`, from a Secret |

## 2. Ports and TLS

| Profile | Listener | Reached as |
|---|---|---|
| `dev` | plain HTTP on `127.0.0.1:8080` | `http://127.0.0.1:8080`, or through a tunnel |
| `production` | HTTPS on `0.0.0.0:443`, TLS 1.3, certificate files | `https://<public host>` |

- In `production` riehub terminates TLS itself. Certificate and key
  are files, reloaded on `SIGHUP`. Getting certificates (ACME client,
  cert-manager) is outside riehub.
- Binding 443 without root: the systemd unit grants
  `CAP_NET_BIND_SERVICE` only. In containers riehub listens on 8443
  and the Service or port mapping publishes 443.
- A TLS-terminating proxy in front is allowed only when its address is
  in `server.trusted_proxies`. Then the proxy can read traffic, and
  the operator has chosen that.

### Testing through a Cloudflare tunnel

During development the instance is reached as `https://hub.rie.my.id`
through `cloudflared`, pointing at the `dev` listener:

```toml
profile = "dev"
[server]
bind = "127.0.0.1:8080"
public_url = "https://hub.rie.my.id"
trusted_proxies = ["127.0.0.1/32"]
client_ip_header = "CF-Connecting-IP"
```

```bash
cloudflared tunnel run --url http://127.0.0.1:8080 <tunnel-name>
```

The tunnel provider terminates TLS and can read the traffic. That is
acceptable for testing with test accounts and is the reason the
tunnel is not the production path. In production the DNS record points
straight at the host and riehub serves 443.

## 3. Standalone

- One static binary, `riehub`. The web UI is inside it.
- `deploy/systemd/riehub.service` runs it as its own user with
  `NoNewPrivileges`, `ProtectSystem=strict`, `ProtectHome`,
  `PrivateTmp`, `PrivateDevices`, `MemoryDenyWriteExecute`,
  `RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX`,
  `SystemCallFilter=@system-service`, `LimitCORE=0`,
  `MemorySwapMax=0`, `CapabilityBoundingSet=CAP_NET_BIND_SERVICE`,
  `ReadWritePaths` limited to the data directory, and
  `LoadCredentialEncrypted` for the instance host key.
- `deploy/launchd/my.rie.riehub.plist` runs it on macOS.
- `deploy/install.sh` installs the binary, the user, the unit and a
  starter config. It refuses to run on anything but Linux or macOS.

## 4. Containers

- Image: multi-stage build, final stage without a shell or package
  manager, runs as a non-root user, read-only root file system, the
  data directory as the only writable mount. Built for `linux/amd64`
  and `linux/arm64`.
- `deploy/compose/compose.yaml`: riehub with SQLite on a named volume,
  and a second file that adds PostgreSQL.
- The image is scanned in CI. A high or critical finding fails the
  build.

## 5. Kubernetes

`deploy/helm/riehub` contains: Deployment, Service, optional Ingress
with TLS passthrough, ConfigMap, Secret references (the instance key
file, the database URL), ServiceAccount without token automount,
NetworkPolicy (ingress only on the service port, egress to the
database and the internet), PodDisruptionBudget, and probes on
`/healthz` and `/readyz`.

Pod security: `runAsNonRoot`, `readOnlyRootFilesystem`,
`allowPrivilegeEscalation: false`, all capabilities dropped,
`seccompProfile: RuntimeDefault`.

## 6. Scaling

Nodes hold no durable state. Sessions and hub keys unlock vaults from
the token alone ([security](security.md)), so any node serves any
request. Shared across nodes through the store: records, counters for
limits and budgets, quota windows, the audit chain, scheduler leases.
Per node and in memory: provider health, circuit breakers, sticky
conversation keys, the unlock cache. Losing a node loses nothing
durable.

SQLite is single node. Running a second node on SQLite is refused at
start through a lease.

## 7. Operations

| Task | How |
|---|---|
| Health | `/healthz` (process up), `/readyz` (store reachable, schema current, coffer open) |
| Metrics | Prometheus text on `server.metrics_bind`. Labels are enums and ids of catalog items only, never user ids or user text |
| Tracing | OTLP export, spans carry the same allowlisted fields as audit events |
| Logs | JSON on stdout, allowlisted fields |
| Backup | `riehub backup`, see [storage](storage.md) |
| Upgrade | replace the binary or image, run `riehub migrate`, start |
| Verify a release | check the signature and the SBOM published with it |
