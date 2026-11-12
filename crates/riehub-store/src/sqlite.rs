//! The SQLite backend (storage spec, section 1): WAL,
//! `synchronous=FULL`, `foreign_keys=ON`, `busy_timeout=5000`, file
//! mode 0600. Every query is parameterised.

use crate::audit::{mac_input, AuditDraft, AuditFilter, AuditHead, AuditRow, ChainMac};
use crate::record::{Cursor, Expect, Filter, Kind, Listed, Page, Record};
use crate::usage::{UsageBucket, UsageGroup, UsageQuery, UsageRow};
use crate::{CounterKey, SchemaVersion, Store};
use async_trait::async_trait;
use riehub_core::error::{Error, Result};
use riehub_core::Owner;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};

use riehub_core::clock::Clock;
use std::path::Path;
use std::time::Duration;

const SCHEMA_VERSION: u64 = 1;
const KNOWN_MIGRATIONS: u64 = 1;
const INIT_SQL: &str = include_str!("../migrations/sqlite/0001_init.sql");

#[derive(Debug)]
pub struct SqliteStore {
    pool: SqlitePool,
}

fn owner_to_str(owner: &Owner) -> String {
    owner.to_string()
}

fn owner_from_str(s: &str) -> Result<Owner> {
    if s == "instance" {
        return Ok(Owner::Instance);
    }
    if let Some(id) = s.strip_prefix("user:") {
        let user_id = riehub_core::UserId::parse(id)?;
        return Ok(Owner::User(user_id));
    }
    Err(Error::Invalid("unknown owner key".to_owned()))
}

impl SqliteStore {
    /// Opens or creates the database file. A new file is created with
    /// mode 0600; an existing file that is group or world readable is
    /// refused (storage spec, section 1).
    pub async fn open(url: &str) -> Result<Self> {
        if !url.starts_with("sqlite://") {
            return Err(Error::Invalid(
                "database.url: the sqlite backend needs a sqlite:// url".to_owned(),
            ));
        }
        let path = &url["sqlite://".len()..];
        if path.is_empty() {
            return Err(Error::Invalid("database.url: empty sqlite path".to_owned()));
        }
        ensure_file_private(path)?;
        let options = SqliteConnectOptions::new()
            .filename(path)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .foreign_keys(true)
            .busy_timeout(Duration::from_millis(5000));
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await
            .map_err(sqlite_error)?;
        Ok(Self { pool })
    }
}

#[cfg(unix)]
fn ensure_file_private(path: &str) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::fs::PermissionsExt;

    let file = Path::new(path);
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(file)
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let meta = std::fs::metadata(file)
                .map_err(|_| Error::NotFound("database file disappeared".to_owned()))?;
            if !meta.is_file() {
                return Err(Error::Invalid("database url is not a file".to_owned()));
            }
            if meta.permissions().mode() & 0o077 != 0 {
                return Err(Error::Invalid(
                    "database file: group or world readable files are refused".to_owned(),
                ));
            }
            Ok(())
        }
        Err(_) => Err(Error::Invalid(
            "database file: the parent directory is not writable".to_owned(),
        )),
    }
}

#[cfg(not(unix))]
fn ensure_file_private(_path: &str) -> Result<()> {
    Ok(())
}

fn sqlite_error(e: sqlx::Error) -> Error {
    match &e {
        sqlx::Error::Database(db) => {
            let raw = db.message();
            if raw.contains("UNIQUE constraint failed") {
                return Error::Conflict("the id or lookup already exists".to_owned());
            }
            Error::Internal(None)
        }
        _ => Error::Internal(None),
    }
}

struct StoredRecord {
    owner: String,
    kind: String,
    id: String,
    parent: Option<String>,
    lookup: Option<String>,
    status: Option<String>,
    sort: i64,
    meta: String,
    data: Option<Vec<u8>>,
    created_at: i64,
    updated_at: i64,
}

fn record_to_row(rec: &Record) -> Result<StoredRecord> {
    if !rec.meta.is_object() {
        return Err(Error::Invalid(
            "record meta must be a JSON object".to_owned(),
        ));
    }
    let meta = serde_json::to_string(&rec.meta).map_err(|_| Error::Internal(None))?;
    Ok(StoredRecord {
        owner: owner_to_str(&rec.owner),
        kind: rec.kind.as_str().to_owned(),
        id: rec.id.clone(),
        parent: rec.parent.clone(),
        lookup: rec.lookup.clone(),
        status: rec.status.clone(),
        sort: rec.sort,
        meta,
        data: rec.data.clone(),
        created_at: rec.created_at,
        updated_at: rec.updated_at,
    })
}

fn row_to_record(row: sqlx::sqlite::SqliteRow) -> Result<Record> {
    use sqlx::Row;
    let owner: String = row.try_get("owner").map_err(sqlite_error)?;
    let kind: String = row.try_get("kind").map_err(sqlite_error)?;
    let meta: String = row.try_get("meta").map_err(sqlite_error)?;
    let id: String = row.try_get("id").map_err(sqlite_error)?;
    let parent: Option<String> = row.try_get("parent").map_err(sqlite_error)?;
    let lookup: Option<String> = row.try_get("lookup").map_err(sqlite_error)?;
    let status: Option<String> = row.try_get("status").map_err(sqlite_error)?;
    let sort: i64 = row.try_get("sort").map_err(sqlite_error)?;
    let data: Option<Vec<u8>> = row.try_get("data").map_err(sqlite_error)?;
    let version: i64 = row.try_get("version").map_err(sqlite_error)?;
    let created_at: i64 = row.try_get("created_at").map_err(sqlite_error)?;
    let updated_at: i64 = row.try_get("updated_at").map_err(sqlite_error)?;
    Ok(Record {
        owner: owner_from_str(&owner)?,
        kind: Kind::parse(&kind)?,
        id,
        parent,
        lookup,
        status,
        sort,
        meta: serde_json::from_str(&meta).map_err(|_| Error::Internal(None))?,
        data,
        version: version.max(0) as u64,
        created_at,
        updated_at,
    })
}

const RECORD_COLS: &str =
    "owner, kind, id, parent, lookup, status, sort, meta, data, version, created_at, updated_at";

const GET_SQL: &str = "SELECT owner, kind, id, parent, lookup, status, sort, meta, data, version, created_at, updated_at FROM records WHERE owner = ? AND kind = ? AND id = ?";
const GET_BY_LOOKUP_SQL: &str = "SELECT owner, kind, id, parent, lookup, status, sort, meta, data, version, created_at, updated_at FROM records WHERE owner = ? AND kind = ? AND lookup = ?";

#[async_trait]
impl Store for SqliteStore {
    async fn get(&self, owner: &Owner, kind: Kind, id: &str) -> Result<Option<Record>> {
        let row = sqlx::query(GET_SQL)
            .bind(owner_to_str(owner))
            .bind(kind.as_str())
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlite_error)?;
        match row {
            Some(row) => Ok(Some(row_to_record(row)?)),
            None => Ok(None),
        }
    }

    async fn get_by_lookup(
        &self,
        owner: &Owner,
        kind: Kind,
        lookup: &str,
    ) -> Result<Option<Record>> {
        let row = sqlx::query(GET_BY_LOOKUP_SQL)
            .bind(owner_to_str(owner))
            .bind(kind.as_str())
            .bind(lookup)
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlite_error)?;
        match row {
            Some(row) => Ok(Some(row_to_record(row)?)),
            None => Ok(None),
        }
    }

    async fn put(&self, rec: &Record, expected: Expect) -> Result<u64> {
        let row = record_to_row(rec)?;
        let mut tx = self.pool.begin().await.map_err(sqlite_error)?;

        match expected {
            Expect::New => {
                let sql = "INSERT INTO records (owner, kind, id, parent, lookup, status, sort, meta, data, version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)";
                let result = sqlx::query(sql)
                    .bind(&row.owner)
                    .bind(&row.kind)
                    .bind(&row.id)
                    .bind(&row.parent)
                    .bind(&row.lookup)
                    .bind(&row.status)
                    .bind(row.sort)
                    .bind(&row.meta)
                    .bind(&row.data)
                    .bind(row.created_at)
                    .bind(row.updated_at)
                    .execute(&mut *tx)
                    .await;
                match result {
                    Ok(_) => {
                        tx.commit().await.map_err(sqlite_error)?;
                        Ok(1)
                    }
                    Err(e) => Err(sqlite_error(e)),
                }
            }
            Expect::Version(expected_version) => {
                let sql = "UPDATE records SET parent = ?, lookup = ?, status = ?, sort = ?, meta = ?, data = ?, version = ?, updated_at = ? WHERE owner = ? AND kind = ? AND id = ? AND version = ?";
                let result = sqlx::query(sql)
                    .bind(&row.parent)
                    .bind(&row.lookup)
                    .bind(&row.status)
                    .bind(row.sort)
                    .bind(&row.meta)
                    .bind(&row.data)
                    .bind(expected_version as i64 + 1)
                    .bind(row.updated_at)
                    .bind(&row.owner)
                    .bind(&row.kind)
                    .bind(&row.id)
                    .bind(expected_version as i64)
                    .execute(&mut *tx)
                    .await
                    .map_err(sqlite_error)?;
                if result.rows_affected() == 0 {
                    return Err(Error::Conflict(
                        "the record version moved or the record is gone".to_owned(),
                    ));
                }
                tx.commit().await.map_err(sqlite_error)?;
                Ok(expected_version + 1)
            }
            Expect::Any => {
                let current: Option<(i64, i64)> = sqlx::query_as(
                    "SELECT version, created_at FROM records WHERE owner = ? AND kind = ? AND id = ?",
                )
                .bind(&row.owner)
                .bind(&row.kind)
                .bind(&row.id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(sqlite_error)?;
                let (version, created_at) = match current {
                    Some(pair) => pair,
                    None => {
                        return Err(Error::NotFound("no such record".to_owned()));
                    }
                };
                let sql = "UPDATE records SET parent = ?, lookup = ?, status = ?, sort = ?, meta = ?, data = ?, version = ?, updated_at = ? WHERE owner = ? AND kind = ? AND id = ?";
                sqlx::query(sql)
                    .bind(&row.parent)
                    .bind(&row.lookup)
                    .bind(&row.status)
                    .bind(row.sort)
                    .bind(&row.meta)
                    .bind(&row.data)
                    .bind(version + 1)
                    .bind(row.updated_at)
                    .bind(&row.owner)
                    .bind(&row.kind)
                    .bind(&row.id)
                    .execute(&mut *tx)
                    .await
                    .map_err(sqlite_error)?;
                let _ = created_at;
                tx.commit().await.map_err(sqlite_error)?;
                Ok((version + 1).max(0) as u64)
            }
        }
    }

    async fn delete(&self, owner: &Owner, kind: Kind, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM records WHERE owner = ? AND kind = ? AND id = ?")
            .bind(owner_to_str(owner))
            .bind(kind.as_str())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(sqlite_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn list(
        &self,
        owner: &Owner,
        kind: Kind,
        filter: &Filter,
        page: &Page,
    ) -> Result<Listed> {
        let limit = page.limit.max(1);
        let mut qb = sqlx::query_builder::QueryBuilder::new("SELECT ");
        qb.push(RECORD_COLS)
            .push(" FROM records WHERE owner = ")
            .push_bind(owner_to_str(owner))
            .push(" AND kind = ")
            .push_bind(kind.as_str());
        if let Some(parent) = &filter.parent {
            qb.push(" AND parent = ").push_bind(parent);
        }
        if let Some(status) = &filter.status {
            qb.push(" AND status = ").push_bind(status);
        }
        if let Some(min) = filter.sort_min {
            qb.push(" AND sort >= ").push_bind(min);
        }
        if let Some(max) = filter.sort_max {
            qb.push(" AND sort <= ").push_bind(max);
        }
        if let Some(after) = &page.after {
            qb.push(" AND (sort, id) > (")
                .push_bind(after.sort)
                .push(", ")
                .push_bind(&after.id)
                .push(")");
        }
        qb.push(" ORDER BY sort ASC, id ASC LIMIT ")
            .push_bind(limit as i64 + 1);

        let rows = qb
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(sqlite_error)?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row_to_record(row)?);
        }
        let next = if items.len() > limit as usize {
            items.truncate(limit as usize);
            let last = items.last().ok_or_else(|| Error::Internal(None))?;
            Some(Cursor {
                sort: last.sort,
                id: last.id.clone(),
            })
        } else {
            None
        };
        Ok(Listed { items, next })
    }

    async fn delete_owner(&self, owner: &Owner) -> Result<u64> {
        let result = sqlx::query("DELETE FROM records WHERE owner = ?")
            .bind(owner_to_str(owner))
            .execute(&self.pool)
            .await
            .map_err(sqlite_error)?;
        Ok(result.rows_affected() as u64)
    }

    async fn audit_append(&self, events: Vec<AuditDraft>, mac: &dyn ChainMac) -> Result<AuditHead> {
        if events.is_empty() {
            return Err(Error::Invalid(
                "an audit batch must not be empty".to_owned(),
            ));
        }
        let mut conn = self.pool.acquire().await.map_err(sqlite_error)?;
        let result = append_chain(&mut conn, events, mac).await;
        if result.is_err() {
            let _ = sqlx::raw_sql("ROLLBACK").execute(&mut *conn).await;
            return result;
        }
        sqlx::raw_sql("COMMIT")
            .execute(&mut *conn)
            .await
            .map_err(sqlite_error)?;
        result
    }

    async fn audit_read(
        &self,
        from_seq: u64,
        limit: u32,
        filter: &AuditFilter,
    ) -> Result<Vec<AuditRow>> {
        // The string is built only from the fixed fragments below; every
        // value is bound, so the query is injection-free.
        let mut sql = "SELECT seq, ts, kind, event, mac FROM audit WHERE seq >= ?".to_owned();
        if filter.kind.is_some() {
            sql.push_str(" AND kind = ?");
        }
        if filter.from_ts.is_some() {
            sql.push_str(" AND ts >= ?");
        }
        if filter.to_ts.is_some() {
            sql.push_str(" AND ts <= ?");
        }
        sql.push_str(" ORDER BY seq ASC LIMIT ?");
        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(from_seq as i64);
        if let Some(kind) = &filter.kind {
            query = query.bind(kind);
        }
        if let Some(from) = filter.from_ts {
            query = query.bind(from);
        }
        if let Some(to) = filter.to_ts {
            query = query.bind(to);
        }
        query = query.bind(limit.max(1) as i64);
        let rows = query.fetch_all(&self.pool).await.map_err(sqlite_error)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            use sqlx::Row;
            let seq: i64 = row.try_get("seq").map_err(sqlite_error)?;
            let ts: i64 = row.try_get("ts").map_err(sqlite_error)?;
            let kind: String = row.try_get("kind").map_err(sqlite_error)?;
            let event: String = row.try_get("event").map_err(sqlite_error)?;
            let mac: Vec<u8> = row.try_get("mac").map_err(sqlite_error)?;
            out.push(AuditRow {
                seq: seq.max(0) as u64,
                ts,
                kind: kind.clone(),
                event: serde_json::from_str(&event).map_err(|_| Error::Internal(None))?,
                mac,
            });
        }
        Ok(out)
    }

    async fn audit_prune(&self, through_seq: u64) -> Result<u64> {
        let result = sqlx::query("DELETE FROM audit WHERE seq <= ?")
            .bind(through_seq as i64)
            .execute(&self.pool)
            .await
            .map_err(sqlite_error)?;
        Ok(result.rows_affected() as u64)
    }

    async fn counter_add(
        &self,
        key: &CounterKey,
        window: i64,
        delta: i64,
        expires_at: i64,
    ) -> Result<i64> {
        let sql = "INSERT INTO counters (owner, name, window, value, expires_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(owner, name, window) DO UPDATE SET value = value + ?, expires_at = ? RETURNING value";
        let (value,): (i64,) = sqlx::query_as(sql)
            .bind(owner_to_str(&key.owner))
            .bind(&key.name)
            .bind(window)
            .bind(delta)
            .bind(expires_at)
            .bind(delta)
            .bind(expires_at)
            .fetch_one(&self.pool)
            .await
            .map_err(sqlite_error)?;
        Ok(value)
    }

    async fn counter_get(&self, key: &CounterKey, window: i64) -> Result<i64> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT value FROM counters WHERE owner = ? AND name = ? AND window = ?",
        )
        .bind(owner_to_str(&key.owner))
        .bind(&key.name)
        .bind(window)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlite_error)?;
        Ok(row.map(|pair| pair.0).unwrap_or(0))
    }

    async fn usage_insert(&self, row: &UsageRow) -> Result<()> {
        sqlx::query(
            "INSERT INTO usage (ts, owner, user_id, team, hub_key, project, provider_kind, model, status, tokens_in, tokens_out, cost_micros) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(row.ts)
        .bind(owner_to_str(&row.owner))
        .bind(&row.user)
        .bind(&row.team)
        .bind(&row.hub_key)
        .bind(&row.project)
        .bind(&row.provider_kind)
        .bind(&row.model)
        .bind(&row.status)
        .bind(row.tokens_in)
        .bind(row.tokens_out)
        .bind(row.cost_micros)
        .execute(&self.pool)
        .await
        .map_err(sqlite_error)?;
        Ok(())
    }

    async fn usage_query(&self, q: &UsageQuery) -> Result<Vec<UsageBucket>> {
        if q.group_by.is_empty() {
            return Err(Error::Invalid(
                "usage_query: group_by must name at least one field".to_owned(),
            ));
        }
        let mut select = Vec::new();
        let mut group = Vec::new();
        for g in &q.group_by {
            let (expr, key) = match g {
                UsageGroup::Hour => ("(ts / 3600000) * 3600000", "hour"),
                UsageGroup::Day => ("(ts / 86400000) * 86400000", "day"),
                UsageGroup::User => ("COALESCE(user_id, '')", "user"),
                UsageGroup::Team => ("COALESCE(team, '')", "team"),
                UsageGroup::HubKey => ("COALESCE(hub_key, '')", "hub_key"),
                UsageGroup::Project => ("COALESCE(project, '')", "project"),
                UsageGroup::ProviderKind => ("COALESCE(provider_kind, '')", "provider_kind"),
                UsageGroup::Model => ("COALESCE(model, '')", "model"),
                UsageGroup::Status => ("COALESCE(status, '')", "status"),
            };
            select.push(format!("{expr} AS {key}"));
            group.push(key);
        }
        select.push("COUNT(*) AS requests".to_owned());
        select.push("SUM(tokens_in) AS tokens_in".to_owned());
        select.push("SUM(tokens_out) AS tokens_out".to_owned());
        select.push("SUM(cost_micros) AS cost_micros".to_owned());

        let mut sql = format!(
            "SELECT {} FROM usage WHERE owner = ? AND ts >= ? AND ts <= ?",
            select.join(", ")
        );
        if q.status.is_some() {
            sql.push_str(" AND status = ?");
        }
        if q.model.is_some() {
            sql.push_str(" AND model = ?");
        }
        if q.user.is_some() {
            sql.push_str(" AND user_id = ?");
        }
        sql.push_str(&format!(
            " GROUP BY {} ORDER BY {}",
            group.join(", "),
            group.join(", ")
        ));

        // The string is built only from the fixed expression table above;
        // every value is bound, so the query is injection-free.
        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(owner_to_str(&q.owner))
            .bind(q.from_ts)
            .bind(q.to_ts);
        if let Some(status) = &q.status {
            query = query.bind(status);
        }
        if let Some(model) = &q.model {
            query = query.bind(model);
        }
        if let Some(user) = &q.user {
            query = query.bind(user);
        }

        let rows = query.fetch_all(&self.pool).await.map_err(sqlite_error)?;
        let mut buckets = Vec::with_capacity(rows.len());
        for row in rows {
            use sqlx::Row;
            let mut keys = Vec::with_capacity(q.group_by.len());
            let mut bucket_start = 0;
            for g in &q.group_by {
                match g {
                    UsageGroup::Hour | UsageGroup::Day => {
                        bucket_start = row.try_get(gname(g)).map_err(sqlite_error)?;
                    }
                    _ => {
                        let key = row.try_get::<String, _>(gname(g)).map_err(sqlite_error)?;
                        keys.push(key);
                    }
                }
            }
            buckets.push(UsageBucket {
                bucket_start,
                keys,
                requests: row.try_get("requests").map_err(sqlite_error)?,
                tokens_in: row.try_get("tokens_in").map_err(sqlite_error)?,
                tokens_out: row.try_get("tokens_out").map_err(sqlite_error)?,
                cost_micros: row.try_get("cost_micros").map_err(sqlite_error)?,
            });
        }
        Ok(buckets)
    }

    async fn lease_acquire(&self, name: &str, holder: &str, ttl_ms: i64) -> Result<bool> {
        let now = riehub_core::SystemClock.now_millis();
        let expires_at = now + ttl_ms;
        let sql = "INSERT INTO leases (name, holder, expires_at) VALUES (?, ?, ?) ON CONFLICT(name) DO UPDATE SET holder = ?, expires_at = ? WHERE leases.expires_at <= ? OR leases.holder = ? RETURNING holder";
        let row: Option<(String,)> = sqlx::query_as(sql)
            .bind(name)
            .bind(holder)
            .bind(expires_at)
            .bind(holder)
            .bind(expires_at)
            .bind(now)
            .bind(holder)
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlite_error)?;
        Ok(row.map(|pair| pair.0 == holder).unwrap_or(false))
    }

    async fn lease_release(&self, name: &str, holder: &str) -> Result<()> {
        sqlx::query("DELETE FROM leases WHERE name = ? AND holder = ?")
            .bind(name)
            .bind(holder)
            .execute(&self.pool)
            .await
            .map_err(sqlite_error)?;
        Ok(())
    }

    async fn migrate(&self) -> Result<SchemaVersion> {
        let mut tx = self.pool.begin().await.map_err(sqlite_error)?;
        sqlx::raw_sql("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)")
            .execute(&mut *tx)
            .await
            .map_err(sqlite_error)?;
        let current: Option<(Option<i64>,)> =
            sqlx::query_as("SELECT MAX(version) FROM schema_version")
                .fetch_optional(&mut *tx)
                .await
                .map_err(sqlite_error)?;
        let current = current.and_then(|pair| pair.0).unwrap_or(0).max(0) as u64;
        if current > KNOWN_MIGRATIONS {
            return Err(Error::Conflict(format!(
                "schema version {current} is newer than this build knows ({KNOWN_MIGRATIONS})"
            )));
        }
        if current == 0 {
            sqlx::raw_sql(INIT_SQL)
                .execute(&mut *tx)
                .await
                .map_err(sqlite_error)?;
            sqlx::query("INSERT INTO schema_version (version) VALUES (?)")
                .bind(SCHEMA_VERSION as i64)
                .execute(&mut *tx)
                .await
                .map_err(sqlite_error)?;
        }
        tx.commit().await.map_err(sqlite_error)?;
        Ok(SchemaVersion(current.max(SCHEMA_VERSION)))
    }

    async fn ping(&self) -> Result<()> {
        let (_one,): (i64,) = sqlx::query_as("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(sqlite_error)?;
        Ok(())
    }
}

fn gname(group: &UsageGroup) -> &'static str {
    match group {
        UsageGroup::Hour => "hour",
        UsageGroup::Day => "day",
        UsageGroup::User => "user",
        UsageGroup::Team => "team",
        UsageGroup::HubKey => "hub_key",
        UsageGroup::Project => "project",
        UsageGroup::ProviderKind => "provider_kind",
        UsageGroup::Model => "model",
        UsageGroup::Status => "status",
    }
}

// One serialized chain: the immediate transaction blocks other writers
// until this batch is committed (storage spec, section 3).
async fn append_chain(
    conn: &mut sqlx::sqlite::SqliteConnection,
    events: Vec<AuditDraft>,
    mac: &dyn ChainMac,
) -> Result<AuditHead> {
    sqlx::raw_sql("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .map_err(sqlite_error)?;

    let head: Option<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT seq, mac FROM audit_head WHERE id = 1")
            .fetch_optional(&mut *conn)
            .await
            .map_err(sqlite_error)?;
    let (head_seq, head_mac) = head.unwrap_or((0, Vec::new()));

    let mut seq = head_seq.max(0) as u64;
    let mut prev = head_mac;
    for draft in &events {
        seq += 1;
        if !draft.event.is_object() {
            return Err(Error::Invalid(
                "an audit event must be a JSON object".to_owned(),
            ));
        }
        let event_json = serde_json::to_string(&draft.event).map_err(|_| Error::Internal(None))?;
        let input = mac_input(&prev, seq, draft.ts, &draft.kind, &event_json);
        let mac_bytes = mac.compute(&prev, &input);
        sqlx::query("INSERT INTO audit (seq, ts, kind, event, mac) VALUES (?, ?, ?, ?, ?)")
            .bind(seq as i64)
            .bind(draft.ts)
            .bind(&draft.kind)
            .bind(&event_json)
            .bind(&mac_bytes[..])
            .execute(&mut *conn)
            .await
            .map_err(sqlite_error)?;
        prev = mac_bytes.to_vec();
    }

    sqlx::query(
        "INSERT INTO audit_head (id, seq, mac) VALUES (1, ?, ?) ON CONFLICT(id) DO UPDATE SET seq = ?, mac = ?",
    )
    .bind(seq as i64)
    .bind(&prev)
    .bind(seq as i64)
    .bind(&prev)
    .execute(&mut *conn)
    .await
    .map_err(sqlite_error)?;

    Ok(AuditHead { seq, mac: prev })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use std::future::Future;
    use std::pin::Pin;

    /// Opens a fresh, migrated SQLite store in a new temp directory.
    pub fn factory() -> impl Fn() -> Pin<Box<dyn Future<Output = Result<SqliteStore>> + Send>> {
        || {
            let url = temp_url("conf");
            Box::pin(async move {
                let store = SqliteStore::open(&url).await?;
                store.migrate().await?;
                Ok(store)
            }) as Pin<Box<dyn Future<Output = Result<SqliteStore>> + Send>>
        }
    }

    #[tokio::test]
    async fn the_conformance_suite_passes_on_sqlite() {
        let make = factory();
        crate::conformance::assert_green(&make).await;
    }

    fn temp_url(tag: &str) -> String {
        let dir = std::env::temp_dir().join(format!(
            "riehub-store-{}-{}",
            tag,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        format!("sqlite://{}/riehub.db", dir.display())
    }

    #[tokio::test]
    async fn db_file_is_0600() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let url = temp_url("mode");
            let store = SqliteStore::open(&url).await.unwrap();
            store.ping().await.unwrap();
            let path = &url["sqlite://".len()..];
            let mode = std::fs::metadata(path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[tokio::test]
    async fn refuses_world_readable_db() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let url = temp_url("world");
            let path = &url["sqlite://".len()..];
            std::fs::write(path, b"not a database but the mode is what counts").unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).unwrap();
            let result = SqliteStore::open(&url).await;
            assert!(result.is_err(), "a world readable database file is refused");
        }
    }

    #[tokio::test]
    async fn migrate_is_idempotent() {
        let url = temp_url("migrate");
        let store = SqliteStore::open(&url).await.unwrap();
        let first = store.migrate().await.unwrap();
        let second = store.migrate().await.unwrap();
        assert_eq!(first, SchemaVersion(1));
        assert_eq!(second, SchemaVersion(1));
        store.ping().await.unwrap();
    }

    #[tokio::test]
    async fn newer_schema_is_refused() {
        let url = temp_url("newer");
        let store = SqliteStore::open(&url).await.unwrap();
        store.migrate().await.unwrap();
        sqlx::query("UPDATE schema_version SET version = 999")
            .execute(&store.pool)
            .await
            .unwrap();
        let result = store.migrate().await;
        assert!(result.is_err(), "a newer schema must be refused");
    }
}
