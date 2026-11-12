//! Tests for the audit crate, including the tamper detection doubles.

// Integration tests: unwraps are the failure mode under test.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use riehub_audit::event::{Actor, AuditAction, AuditEvent, Event, Outcome};
use riehub_audit::{AuditChainMac, AuditLog, VerifyOutcome};
use riehub_core::error::Result;
use riehub_core::Owner;
use riehub_store::audit::{AuditDraft, AuditFilter, AuditHead, AuditRow, ChainMac};
use riehub_store::record::{Expect, Filter, Kind, Listed, Page, Record};
use riehub_store::sqlite::SqliteStore;
use riehub_store::usage::{UsageBucket, UsageQuery, UsageRow};
use riehub_store::{CounterKey, SchemaVersion, Store};
use std::sync::Arc;

fn temp_url(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "riehub-audit-{}-{}",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    format!("sqlite://{}/riehub.db", dir.display())
}

async fn fresh_store(tag: &str) -> SqliteStore {
    let store = SqliteStore::open(&temp_url(tag)).await.unwrap();
    store.migrate().await.unwrap();
    store
}

fn sample(n: u64) -> Event {
    Event {
        ts: 1_700_000_000_000 + n as i64,
        actor: Actor::System,
        action: AuditAction::SystemStart,
        outcome: Outcome::Ok,
        request_id: None,
        target: None,
        ip: None,
        event: AuditEvent::SystemStart {},
    }
}

async fn fresh_log(tag: &str) -> (AuditLog<SqliteStore>, Arc<SqliteStore>) {
    let store = Arc::new(fresh_store(tag).await);
    let mac = AuditChainMac::new(riehub_crypto::Key32::new([3u8; 32]));
    let log = AuditLog::new(store.clone(), mac);
    (log, store)
}

/// How a test double tampers with what `audit_read` returns. This
/// simulates an attacker who can write the database (ADR 0010).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tamper {
    ModifyEvent(u64),
    DeleteRow(u64),
    SwapRows(u64, u64),
}

struct TamperStore {
    inner: SqliteStore,
    tamper: Tamper,
}

impl TamperStore {
    fn tamper_rows(&self, mut rows: Vec<AuditRow>) -> Vec<AuditRow> {
        match self.tamper {
            Tamper::ModifyEvent(seq) => {
                for row in &mut rows {
                    if row.seq == seq {
                        row.event = serde_json::json!({"type": "system_start", "tampered": true});
                    }
                }
                rows
            }
            Tamper::DeleteRow(seq) => {
                rows.retain(|row| row.seq != seq);
                rows
            }
            Tamper::SwapRows(a, b) => {
                let at_a = rows.iter().position(|r| r.seq == a);
                let at_b = rows.iter().position(|r| r.seq == b);
                if let (Some(x), Some(y)) = (at_a, at_b) {
                    rows.swap(x, y);
                }
                rows
            }
        }
    }
}

#[async_trait::async_trait]
impl Store for TamperStore {
    async fn get(&self, owner: &Owner, kind: Kind, id: &str) -> Result<Option<Record>> {
        self.inner.get(owner, kind, id).await
    }
    async fn get_by_lookup(
        &self,
        owner: &Owner,
        kind: Kind,
        lookup: &str,
    ) -> Result<Option<Record>> {
        self.inner.get_by_lookup(owner, kind, lookup).await
    }
    async fn put(&self, rec: &Record, expected: Expect) -> Result<u64> {
        self.inner.put(rec, expected).await
    }
    async fn delete(&self, owner: &Owner, kind: Kind, id: &str) -> Result<bool> {
        self.inner.delete(owner, kind, id).await
    }
    async fn list(
        &self,
        owner: &Owner,
        kind: Kind,
        filter: &Filter,
        page: &Page,
    ) -> Result<Listed> {
        self.inner.list(owner, kind, filter, page).await
    }
    async fn delete_owner(&self, owner: &Owner) -> Result<u64> {
        self.inner.delete_owner(owner).await
    }
    async fn audit_append(&self, events: Vec<AuditDraft>, mac: &dyn ChainMac) -> Result<AuditHead> {
        self.inner.audit_append(events, mac).await
    }
    async fn audit_read(
        &self,
        from_seq: u64,
        limit: u32,
        filter: &AuditFilter,
    ) -> Result<Vec<AuditRow>> {
        let rows = self.inner.audit_read(from_seq, limit, filter).await?;
        Ok(self.tamper_rows(rows))
    }
    async fn audit_prune(&self, through_seq: u64) -> Result<u64> {
        self.inner.audit_prune(through_seq).await
    }
    async fn counter_add(
        &self,
        key: &CounterKey,
        window: i64,
        delta: i64,
        expires_at: i64,
    ) -> Result<i64> {
        self.inner.counter_add(key, window, delta, expires_at).await
    }
    async fn counter_get(&self, key: &CounterKey, window: i64) -> Result<i64> {
        self.inner.counter_get(key, window).await
    }
    async fn usage_insert(&self, row: &UsageRow) -> Result<()> {
        self.inner.usage_insert(row).await
    }
    async fn usage_query(&self, q: &UsageQuery) -> Result<Vec<UsageBucket>> {
        self.inner.usage_query(q).await
    }
    async fn lease_acquire(&self, name: &str, holder: &str, ttl_ms: i64) -> Result<bool> {
        self.inner.lease_acquire(name, holder, ttl_ms).await
    }
    async fn lease_release(&self, name: &str, holder: &str) -> Result<()> {
        self.inner.lease_release(name, holder).await
    }
    async fn migrate(&self) -> Result<SchemaVersion> {
        self.inner.migrate().await
    }
    async fn ping(&self) -> Result<()> {
        self.inner.ping().await
    }
}

#[tokio::test]
async fn chain_verifies_after_1000_appends() {
    let (log, _store) = fresh_log("thousand").await;
    for batch in 0..100u64 {
        let events: Vec<Event> = (0..10).map(|i| sample(batch * 10 + i)).collect();
        log.append(events).await.unwrap();
    }
    let outcome = log.verify(1, None).await.unwrap();
    assert_eq!(outcome, VerifyOutcome::Ok { head_seq: 1000 });
}

#[tokio::test]
async fn verify_detects_modified_event() {
    let store = fresh_store("modified").await;
    let store = Arc::new(TamperStore {
        inner: store,
        tamper: Tamper::ModifyEvent(7),
    });
    let mac = AuditChainMac::new(riehub_crypto::Key32::new([3u8; 32]));
    let log = AuditLog::new(store, mac);
    let events: Vec<Event> = (0..10).map(sample).collect();
    log.append(events).await.unwrap();
    let outcome = log.verify(1, None).await.unwrap();
    assert!(
        matches!(outcome, VerifyOutcome::Broken { at: 7, .. }),
        "the modified event must break the chain: {outcome:?}"
    );
}

#[tokio::test]
async fn verify_detects_deleted_event() {
    let store = fresh_store("deleted").await;
    let store = Arc::new(TamperStore {
        inner: store,
        tamper: Tamper::DeleteRow(4),
    });
    let mac = AuditChainMac::new(riehub_crypto::Key32::new([3u8; 32]));
    let log = AuditLog::new(store, mac);
    let events: Vec<Event> = (0..10).map(sample).collect();
    log.append(events).await.unwrap();
    let outcome = log.verify(1, None).await.unwrap();
    assert!(
        matches!(outcome, VerifyOutcome::Broken { at: 4, .. }),
        "the deleted event must break the chain: {outcome:?}"
    );
}

#[tokio::test]
async fn verify_detects_reordered_events() {
    let store = fresh_store("reordered").await;
    let store = Arc::new(TamperStore {
        inner: store,
        tamper: Tamper::SwapRows(3, 4),
    });
    let mac = AuditChainMac::new(riehub_crypto::Key32::new([3u8; 32]));
    let log = AuditLog::new(store, mac);
    let events: Vec<Event> = (0..10).map(sample).collect();
    log.append(events).await.unwrap();
    let outcome = log.verify(1, None).await.unwrap();
    assert!(
        matches!(outcome, VerifyOutcome::Broken { .. }),
        "reordered events must break the chain: {outcome:?}"
    );
}

#[tokio::test]
async fn verify_detects_wrong_key() {
    let store = fresh_store("wrongkey").await;
    let store = Arc::new(store);
    let mac = AuditChainMac::new(riehub_crypto::Key32::new([3u8; 32]));
    let log = AuditLog::new(store, mac);
    let events: Vec<Event> = (0..10).map(sample).collect();
    log.append(events).await.unwrap();

    let other_key = AuditChainMac::new(riehub_crypto::Key32::new([4u8; 32]));
    let outcome = riehub_audit::verify::verify(log.store().as_ref(), &other_key, 1, None)
        .await
        .unwrap();
    assert!(
        matches!(outcome, VerifyOutcome::Broken { at: 1, .. }),
        "another key must not verify the chain: {outcome:?}"
    );
}

#[tokio::test]
async fn append_fails_when_store_fails() {
    /// A store whose audit writes always fail.
    struct FailingStore(SqliteStore);

    #[async_trait::async_trait]
    impl Store for FailingStore {
        async fn get(&self, owner: &Owner, kind: Kind, id: &str) -> Result<Option<Record>> {
            self.0.get(owner, kind, id).await
        }
        async fn get_by_lookup(
            &self,
            owner: &Owner,
            kind: Kind,
            lookup: &str,
        ) -> Result<Option<Record>> {
            self.0.get_by_lookup(owner, kind, lookup).await
        }
        async fn put(&self, rec: &Record, expected: Expect) -> Result<u64> {
            self.0.put(rec, expected).await
        }
        async fn delete(&self, owner: &Owner, kind: Kind, id: &str) -> Result<bool> {
            self.0.delete(owner, kind, id).await
        }
        async fn list(
            &self,
            owner: &Owner,
            kind: Kind,
            filter: &Filter,
            page: &Page,
        ) -> Result<Listed> {
            self.0.list(owner, kind, filter, page).await
        }
        async fn delete_owner(&self, owner: &Owner) -> Result<u64> {
            self.0.delete_owner(owner).await
        }
        async fn audit_append(
            &self,
            _events: Vec<AuditDraft>,
            _mac: &dyn ChainMac,
        ) -> Result<AuditHead> {
            Err(riehub_core::Error::Unavailable(
                "the audit write failed".to_owned(),
            ))
        }
        async fn audit_read(
            &self,
            from_seq: u64,
            limit: u32,
            filter: &AuditFilter,
        ) -> Result<Vec<AuditRow>> {
            self.0.audit_read(from_seq, limit, filter).await
        }
        async fn audit_prune(&self, through_seq: u64) -> Result<u64> {
            self.0.audit_prune(through_seq).await
        }
        async fn counter_add(
            &self,
            key: &CounterKey,
            window: i64,
            delta: i64,
            expires_at: i64,
        ) -> Result<i64> {
            self.0.counter_add(key, window, delta, expires_at).await
        }
        async fn counter_get(&self, key: &CounterKey, window: i64) -> Result<i64> {
            self.0.counter_get(key, window).await
        }
        async fn usage_insert(&self, row: &UsageRow) -> Result<()> {
            self.0.usage_insert(row).await
        }
        async fn usage_query(&self, q: &UsageQuery) -> Result<Vec<UsageBucket>> {
            self.0.usage_query(q).await
        }
        async fn lease_acquire(&self, name: &str, holder: &str, ttl_ms: i64) -> Result<bool> {
            self.0.lease_acquire(name, holder, ttl_ms).await
        }
        async fn lease_release(&self, name: &str, holder: &str) -> Result<()> {
            self.0.lease_release(name, holder).await
        }
        async fn migrate(&self) -> Result<SchemaVersion> {
            self.0.migrate().await
        }
        async fn ping(&self) -> Result<()> {
            self.0.ping().await
        }
    }

    let store = Arc::new(FailingStore(fresh_store("failing").await));
    let mac = AuditChainMac::new(riehub_crypto::Key32::new([3u8; 32]));
    let log = AuditLog::new(store, mac);
    let result = log.append(vec![sample(0)]).await;
    assert!(result.is_err(), "a failing store must fail the append");
}

#[tokio::test]
async fn append_returns_after_the_batch_is_durable() {
    let (log, store) = fresh_log("durable").await;
    log.append(vec![sample(1)]).await.unwrap();
    // After append returns, the rows must already be readable.
    let rows = store
        .audit_read(1, 100, &AuditFilter::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "append must be durable on return");
}

#[tokio::test]
async fn prune_event_carries_the_resume_mac() {
    let (log, store) = fresh_log("prune").await;
    let events: Vec<Event> = (0..10).map(sample).collect();
    log.append(events).await.unwrap();

    // Retention (ADR 0010): the prune event carries the seq and mac of
    // the last removed row, then the whole prefix goes away.
    let rows = store
        .audit_read(1, 100, &AuditFilter::default())
        .await
        .unwrap();
    let last = rows.last().unwrap();
    let mut last_mac = [0u8; 48];
    last_mac.copy_from_slice(&last.mac);
    let prune = Event {
        ts: 1_700_000_100_000,
        actor: Actor::System,
        action: AuditAction::AuditPrune,
        outcome: Outcome::Ok,
        request_id: None,
        target: None,
        ip: None,
        event: AuditEvent::AuditPrune {
            through_seq: last.seq,
            last_mac,
        },
    };
    log.append(vec![prune]).await.unwrap();
    store.audit_prune(10).await.unwrap();

    let outcome = log.verify(1, None).await.unwrap();
    assert_eq!(
        outcome,
        VerifyOutcome::Ok { head_seq: 11 },
        "verification must resume after a legal prefix prune"
    );
}
