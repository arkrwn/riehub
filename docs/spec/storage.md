# Storage

One narrow trait, several backends. Crate: `riehub-store`. Decision:
ADR 0006.

## 1. Backends

| Backend | Tier | Use | Since |
|---|---|---|---|
| SQLite | 1 | standalone, the default. WAL mode, `synchronous=FULL`, file mode 0600 | 0.0.1 |
| PostgreSQL 14+ | 1 | several nodes, Kubernetes | 0.8.0 |
| MongoDB 6+ | 2 | sites that standardise on it. Also Oracle Autonomous JSON Database through the Oracle Database API for MongoDB | 0.8.0 |

`database.url` picks the backend by scheme: `sqlite://`, `postgres://`,
`mongodb://`. Tier 1 backends are tested on every pull request. Tier 2
is tested in CI against MongoDB. The Oracle path uses the same driver
and is documented as "expected to work, verified by the operator with
`riehub doctor --store`".

Every backend passes the same conformance suite,
`riehub_store::conformance`. A backend that fails one case is not
released.

## 2. Records

Almost everything is a record. This one shape maps to a SQL table and
to a document collection alike.

| Field | Type | Notes |
|---|---|---|
| `owner` | `instance` or `user:<id>` | the tenant. Part of every key |
| `kind` | enum | `user`, `vault`, `session`, `hub_key`, `connection`, `routing_group`, `connector`, `skill`, `chat`, `chat_message`, `workflow`, `run`, ... |
| `id` | string | typed id, `<prefix>_<ULID>` |
| `parent` | optional id | for example the chat of a message |
| `lookup` | optional string | unique per owner and kind. For user text this is `hmac(content_key("index"), text)` in hex, never the text |
| `status` | optional short enum string | filterable |
| `sort` | i64 | ordering inside a list |
| `meta` | JSON object | ids, enums, numbers, booleans and timestamps only |
| `data` | optional bytes | AEAD ciphertext for user-owned kinds ([security](security.md), section 4). Plain JSON for instance-owned configuration |
| `version` | u64 | starts at 1, compare-and-swap on write |
| `created_at`, `updated_at` | UTC milliseconds | |

Rules:

- A user-owned record never has free text outside `data` (S1).
- Instance-owned records (teams, catalog, policies, roles) are admin
  configuration and are stored plain. They contain no user content.
- `UserScope` is the only way to address `owner = user:<id>` from a
  request handler (S5). Its constructor is private to
  `riehub-identity`.

## 3. The trait

```text
trait Store: Send + Sync + 'static {
    // records
    async fn get(&self, owner: &Owner, kind: Kind, id: &str) -> Result<Option<Record>>;
    async fn get_by_lookup(&self, owner: &Owner, kind: Kind, lookup: &str) -> Result<Option<Record>>;
    async fn put(&self, rec: &Record, expected: Expect) -> Result<u64>;   // Expect::New | Expect::Version(n) | Expect::Any
    async fn delete(&self, owner: &Owner, kind: Kind, id: &str) -> Result<bool>;
    async fn list(&self, owner: &Owner, kind: Kind, filter: &Filter, page: &Page) -> Result<Listed>;
    async fn delete_owner(&self, owner: &Owner) -> Result<u64>;           // removes every record of a user

    // audit, one serialized chain
    async fn audit_append(&self, events: Vec<AuditDraft>, mac: &dyn ChainMac) -> Result<AuditHead>;
    async fn audit_read(&self, from_seq: u64, limit: u32, filter: &AuditFilter) -> Result<Vec<AuditRow>>;
    async fn audit_prune(&self, through_seq: u64) -> Result<u64>;

    // counters for rate limits, budgets and throttles
    async fn counter_add(&self, key: &CounterKey, window: i64, delta: i64, expires_at: i64) -> Result<i64>;
    async fn counter_get(&self, key: &CounterKey, window: i64) -> Result<i64>;

    // usage
    async fn usage_insert(&self, row: &UsageRow) -> Result<()>;
    async fn usage_query(&self, q: &UsageQuery) -> Result<Vec<UsageBucket>>;

    // leases for the scheduler
    async fn lease_acquire(&self, name: &str, holder: &str, ttl_ms: i64) -> Result<bool>;
    async fn lease_release(&self, name: &str, holder: &str) -> Result<()>;

    // lifecycle
    async fn migrate(&self) -> Result<SchemaVersion>;
    async fn ping(&self) -> Result<()>;
}
```

- `Filter` is equality on `parent` and `status`, and a range on `sort`.
  Nothing else is queryable, on purpose: it keeps every backend equal.
- `put` with `Expect::Version(n)` fails with `Conflict` when the stored
  version differs. `Expect::New` fails when the id or lookup exists.
- `audit_append` computes each `mac` from the previous head inside one
  transaction (SQLite: immediate transaction. PostgreSQL: an advisory
  lock. MongoDB: a transaction on a head document).
- `counter_add` is atomic and returns the new value.
- `usage_query` groups by the fields in `UsageQuery.group_by` (a fixed
  enum set: hour, day, user, team, hub key, project, provider kind,
  model, status) and always filters by an owner or a permission scope.

Everything above the trait is backend-neutral. No crate other than
`riehub-store` contains SQL or driver calls.

## 4. Migrations

- Each backend keeps numbered migrations in
  `crates/riehub-store/migrations/<backend>/NNNN_name.*`. They only
  move forward.
- `riehub serve` refuses to start on a newer schema than it knows, and
  applies older ones only with `--migrate` or after `riehub migrate`.
- Every release that changes the schema has a test that upgrades a
  database created by the previous release.

## 5. Backup and restore

`riehub backup <file>` writes one archive: a consistent export of every
record, the audit log and usage tables, and the instance coffer. User
data in it is still ciphertext. `riehub restore <file>` loads it into
an empty store. Restoring needs the instance host key or the instance
recovery key. It never needs, and cannot use, a user's key.
