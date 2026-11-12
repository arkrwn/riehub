# M08 Deploy and scale - version 0.8.0

More databases, native TLS on 443, containers, Kubernetes, several
nodes, and packaging for a plain VM or a Mac.

## T-0801 PostgreSQL backend
- Version: 0.8.0
- Depends: T-0707
- Review: security
- Spec: docs/spec/storage.md, docs/adr/0006-storage-trait-and-backends.md
- Paths: crates/riehub-store/src/postgres/, crates/riehub-store/migrations/postgres/, Makefile, .github/workflows/ci.yml

### Build
1. Backend on `sqlx` with the `postgres` feature and `rustls`. TLS to
   the database is required unless the host is loopback.
2. Migrations mirroring the SQLite schema. `audit_append` under an
   advisory lock in one transaction. `counter_add` as an upsert.
3. Run the conformance suite against it. The test reads
   `RIEHUB_TEST_POSTGRES_URL`. When `CI=true` and the variable is
   missing, the test fails. Locally it is skipped with a printed note.
4. CI starts a PostgreSQL service container. `make test-postgres`
   starts one with Docker locally.

### Accept
- The full conformance suite passes on PostgreSQL.
- `audit_append_is_gapless_under_concurrency` with 8 connections.
- `counter_add_is_atomic_across_connections`.
- `connection_without_tls_to_remote_host_is_refused`.
- The whole server test suite passes with `database.url` pointing at
  PostgreSQL (`make test-postgres`).

### Proof
`make test-postgres`

## T-0802 MongoDB backend
- Version: 0.8.0
- Depends: T-0801
- Review: security
- Spec: docs/spec/storage.md, docs/adr/0006-storage-trait-and-backends.md
- Paths: crates/riehub-store/src/mongodb/, Makefile, .github/workflows/ci.yml, docs/guides/

### Build
1. Backend on the official `mongodb` crate with `rustls`. Collections
   `records`, `audit`, `counters`, `usage`, `usage_buckets`, `leases`,
   with the unique indexes that give the trait's guarantees.
2. `audit_append` in a transaction on a head document (needs a replica
   set, check at start and say so in the error). `counter_add` with
   `$inc` and upsert.
3. Conformance suite with `RIEHUB_TEST_MONGODB_URL`, same CI rule as
   PostgreSQL. `make test-mongodb`.
4. `riehub doctor --store` runs a short self-check of every trait
   guarantee against the configured store.
5. Docs page `docs/guides/databases.md`: all three backends, and the
   Oracle Autonomous JSON Database path through the Oracle Database
   API for MongoDB, stated as expected to work and verified with
   `riehub doctor --store`.

### Accept
- The full conformance suite passes on MongoDB.
- `standalone_mongodb_without_replica_set_is_refused_with_clear_error`.
- `doctor_store_reports_each_guarantee`.

### Proof
`make test-mongodb`

## T-0803 Native TLS and trusted proxies
- Version: 0.8.0
- Depends: T-0707
- Review: security
- Spec: docs/spec/deploy.md section 2, docs/adr/0015-ports-tls-and-tunnel.md, docs/spec/api.md section 3
- Paths: crates/riehub-server/src/tls.rs, crates/riehub-server/

### Build
1. `server.tls.mode = "files"`: `rustls` listener, TLS 1.3 only,
   ALPN `h2` and `http/1.1`. Key file must be mode 0600.
2. Reload certificate and key on `SIGHUP` without dropping
   connections. A bad new certificate keeps the old one and logs an
   error.
3. Client IP: taken from `server.client_ip_header` only when the peer
   address is in `server.trusted_proxies`, otherwise the peer address.
4. HSTS on HTTPS responses. The production start-up checks of the
   spec.

### Accept
- `tls12_client_is_refused`, `tls13_client_connects`.
- `sighup_reloads_certificate`, `bad_reload_keeps_old_certificate`.
- `key_file_with_loose_mode_is_refused`.
- `client_ip_header_ignored_from_untrusted_peer`.
- `client_ip_header_used_from_trusted_peer`.
- `production_refuses_plain_http_without_trusted_proxy`.

### Proof
`cargo test -p riehub-server tls`

## T-0804 Container image and Compose
- Version: 0.8.0
- Depends: T-0803
- Review: security
- Spec: docs/spec/deploy.md section 4
- Paths: deploy/docker/, deploy/compose/, Makefile, .github/workflows/

### Build
1. `deploy/docker/Dockerfile`: multi-stage, builds the web UI and a
   static binary (musl target), final stage `scratch` or distroless
   static, non-root user, `EXPOSE 8443`.
2. Multi-arch build for `linux/amd64` and `linux/arm64`.
3. `deploy/compose/compose.yaml` (SQLite on a volume) and
   `compose.postgres.yaml`. Read-only root file system, dropped
   capabilities, `no-new-privileges`, the instance key as a secret.
4. CI job: build the image, start it, run a smoke test, scan it with
   Trivy, fail on high or critical.
5. `make image` and `make image-smoke`.

### Accept
- `make image-smoke`: the container starts with a read-only root file
  system as a non-root user and answers `/healthz` over TLS.
- The image has no shell (`docker run --entrypoint sh` fails).
- The scan reports no high or critical finding.
- Image size is recorded in `docs/guides/docker.md`.

### Proof
`make image-smoke`

## T-0805 Helm chart
- Version: 0.8.0
- Depends: T-0804, T-0801
- Review: security
- Spec: docs/spec/deploy.md section 5
- Paths: deploy/helm/riehub/, Makefile, .github/workflows/

### Build
1. The chart with every object named in the spec and the pod security
   settings listed there. Values for image, replicas, database URL
   secret, instance key secret, TLS secret, ingress, resources.
2. `helm lint` and `helm template` piped through `kubeconform` in
   `make helm-check`.
3. `make k8s-e2e`: create a `kind` cluster, install PostgreSQL and the
   chart with 2 replicas, run the smoke journey through the Service,
   delete the cluster. Runs in CI.
4. Add `helm`, `kind`, `kubectl`, `kubeconform` to
   `scripts/install-tools.sh` as optional tools needed by this target.

### Accept
- `make helm-check` passes.
- `make k8s-e2e` passes: 2 replicas ready, login on one pod is valid
  on the other, a rolling restart drops no request in a 30 s loop.
- The NetworkPolicy blocks pod-to-pod traffic on other ports (tested
  in the kind run when the CNI supports it, otherwise asserted on the
  rendered manifest).

### Proof
`make helm-check && make k8s-e2e`

## T-0806 Multi-node correctness
- Version: 0.8.0
- Depends: T-0801
- Review: security
- Spec: docs/spec/deploy.md section 6, docs/adr/0012-stateless-nodes.md
- Paths: crates/riehub-server/tests/multinode.rs, crates/riehub-server/

### Build
1. A test harness that starts 3 servers on one PostgreSQL.
2. A start-up lease that refuses a second node on SQLite.
3. Fix anything the tests below find. Do not add shared in-memory
   state.

### Accept
- `session_and_hub_key_work_on_every_node`.
- `rate_limit_and_budget_hold_across_nodes` (the total admitted is
  within one request of the limit under concurrent load on 3 nodes).
- `revocation_on_one_node_applies_on_all_within_one_request`.
- `scheduler_fires_once_across_nodes`.
- `audit_chain_is_one_valid_chain_across_nodes`.
- `oauth_refresh_happens_once_across_nodes`.
- `killing_a_node_mid_request_loses_no_durable_state`.
- `second_node_on_sqlite_is_refused`.

### Proof
`make test-postgres`

## T-0807 Standalone packaging, tunnel guide, backup
- Version: 0.8.0
- Depends: T-0803
- Review: security
- Spec: docs/spec/deploy.md sections 2-3, 7, docs/spec/storage.md section 5
- Paths: deploy/systemd/, deploy/launchd/, deploy/install.sh, crates/riehub-server/src/cli/, docs/guides/

### Build
1. `deploy/systemd/riehub.service` with every directive in the spec.
   `systemd-analyze security` exposure score recorded in the guide.
2. `deploy/launchd/my.rie.riehub.plist`.
3. `deploy/install.sh` for Linux and macOS. It refuses other systems.
   It checks the release signature before installing.
4. `riehub backup` and `riehub restore` as specified.
5. Guides: `docs/guides/install-linux.md`, `install-macos.md`,
   `tunnel.md` (the Cloudflare tunnel setup for `hub.rie.my.id` from
   the spec, with the warning about who can read traffic),
   `backup.md`.

### Accept
- `backup_then_restore_into_empty_store_round_trips`.
- `backup_contains_no_plaintext_user_data` (canary).
- `restore_without_instance_key_fails`.
- `systemd_unit_passes_systemd_analyze_verify` (skipped off Linux with
  a note).
- `install_sh_refuses_unsupported_os` (run with a faked `uname`).
- `shellcheck` passes on `deploy/install.sh` and every script.

### Proof
`cargo test -p riehub-server backup`

## T-0808 Upgrade path
- Version: 0.8.0
- Depends: T-0802
- Review: standard
- Type: test
- Spec: docs/spec/storage.md section 4
- Paths: crates/riehub-store/tests/upgrade.rs, crates/riehub-store/tests/snapshots/

### Build
1. Keep a small database snapshot per backend from each released
   version under `tests/snapshots/` (created by a script from the
   tagged release, containing only synthetic data).
2. A test that opens each snapshot with the current code, migrates,
   and runs a read of every record kind.
3. Document the rule: a schema change adds a snapshot of the release
   before it.

### Accept
- `snapshots_from_every_release_migrate_and_read`.
- `downgrade_is_refused_with_clear_error`.

### Proof
`cargo test -p riehub-store --test upgrade`

## T-0899 Release 0.8.0
- Version: 0.8.0
- Depends: T-0801, T-0802, T-0803, T-0804, T-0805, T-0806, T-0807, T-0808
- Review: standard
- Type: chore
- Spec: SDD.md section "Releases"
- Paths: Cargo.toml, web/package.json, CHANGELOG.md, README.md, docs/, .github/workflows/

### Build
1. Version `0.8.0` in `Cargo.toml` and `web/package.json`.
2. `CHANGELOG.md` section `## [0.8.0] - <today>`.
3. A release workflow triggered by a `v*` tag: build binaries for the
   four targets and the image, generate a CycloneDX SBOM, sign with
   cosign keyless, attach checksums. It publishes a draft release.
4. After `finish`, run `factory/factory.sh release 0.8.0`.

### Accept
- The release workflow passes `actionlint`.
- `factory/factory.sh release 0.8.0` prints `RELEASED v0.8.0`.

### Proof
`make gate`
