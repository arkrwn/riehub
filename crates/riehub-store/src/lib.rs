//! riehub-store: the `Store` trait and its backends (ADR 0006). No
//! SQL or driver call exists outside this crate.

#![forbid(unsafe_code)]

pub mod audit;
pub mod record;
pub mod sqlite;
pub mod usage;

#[cfg(any(feature = "conformance", test))]
pub mod conformance;

use crate::audit::{AuditDraft, AuditFilter, AuditHead, AuditRow, ChainMac};
use crate::record::{Expect, Filter, Listed, Page, Record};
use crate::usage::{UsageBucket, UsageQuery, UsageRow};
use async_trait::async_trait;
use riehub_core::error::Result;
use riehub_core::Owner;

/// One narrow persistence trait (storage spec, section 3).
#[async_trait]
pub trait Store: Send + Sync + 'static {
    // records
    async fn get(&self, owner: &Owner, kind: record::Kind, id: &str) -> Result<Option<Record>>;
    async fn get_by_lookup(
        &self,
        owner: &Owner,
        kind: record::Kind,
        lookup: &str,
    ) -> Result<Option<Record>>;
    async fn put(&self, rec: &Record, expected: Expect) -> Result<u64>;
    async fn delete(&self, owner: &Owner, kind: record::Kind, id: &str) -> Result<bool>;
    async fn list(
        &self,
        owner: &Owner,
        kind: record::Kind,
        filter: &Filter,
        page: &Page,
    ) -> Result<Listed>;
    async fn delete_owner(&self, owner: &Owner) -> Result<u64>;

    // audit, one serialized chain
    async fn audit_append(&self, events: Vec<AuditDraft>, mac: &dyn ChainMac) -> Result<AuditHead>;
    async fn audit_read(
        &self,
        from_seq: u64,
        limit: u32,
        filter: &AuditFilter,
    ) -> Result<Vec<AuditRow>>;
    async fn audit_prune(&self, through_seq: u64) -> Result<u64>;

    // counters for rate limits, budgets and throttles
    async fn counter_add(
        &self,
        key: &CounterKey,
        window: i64,
        delta: i64,
        expires_at: i64,
    ) -> Result<i64>;
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

/// A counter identity: one owner, one named limit.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CounterKey {
    pub owner: Owner,
    pub name: String,
}

impl CounterKey {
    pub fn new(owner: Owner, name: impl Into<String>) -> Self {
        Self {
            owner,
            name: name.into(),
        }
    }
}

/// The store schema version. Versions only move forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SchemaVersion(pub u64);

/// The backends `database.url` can name (storage spec, section 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Sqlite,
    Postgres,
    MongoDb,
}

impl Backend {
    pub fn from_scheme(url: &str) -> Result<Backend> {
        if url.starts_with("sqlite://") {
            Ok(Backend::Sqlite)
        } else if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            Ok(Backend::Postgres)
        } else if url.starts_with("mongodb://") || url.starts_with("mongodb+srv://") {
            Ok(Backend::MongoDb)
        } else {
            Err(riehub_core::Error::Invalid(
                "database.url: unknown scheme, use sqlite://, postgres:// or mongodb://".to_owned(),
            ))
        }
    }
}

/// Opens the backend named by the url scheme (storage spec, section 1).
pub async fn open(url: &str) -> Result<sqlite::SqliteStore> {
    match Backend::from_scheme(url)? {
        Backend::Sqlite => sqlite::SqliteStore::open(url).await,
        Backend::Postgres | Backend::MongoDb => Err(riehub_core::Error::Unavailable(
            "this backend ships in a later release".to_owned(),
        )),
    }
}
