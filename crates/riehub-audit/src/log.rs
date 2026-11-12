//! The audit log: appends through the store, batches for up to 10 ms,
//! and verifies the chain (ADR 0010).

use crate::event::Event;
use crate::{chain_input, AuditChainMac};
use riehub_core::error::{Error, Result};
use riehub_crypto::Key32;
use riehub_store::audit::{AuditDraft, AuditHead, AuditRow, ChainMac};
use riehub_store::Store;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

const BATCH_WINDOW: Duration = Duration::from_millis(10);
const READ_PAGE: u32 = 1000;

/// Where the audit key comes from. The server wires a file source
/// until the instance coffer exists (T-0102).
pub trait KeySource: Send + Sync {
    fn load(&self) -> Result<Key32>;
}

struct Message {
    events: Vec<Event>,
    reply: oneshot::Sender<Result<AuditHead>>,
}

/// A batched, durable writer over one store. `append` returns only
/// after its events are committed; appends that arrive inside the
/// 10 ms window share one commit. A store failure comes back to every
/// waiter as `Err`; nothing is dropped silently.
#[derive(Debug)]
pub struct AuditLog<S: Store> {
    store: Arc<S>,
    mac: AuditChainMac,
    tx: mpsc::UnboundedSender<Message>,
}

impl<S: Store> AuditLog<S> {
    pub fn new(store: Arc<S>, mac: AuditChainMac) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let store_task = store.clone();
        let mac_task = mac.clone();
        tokio::spawn(writer(store_task, mac_task, rx));
        Self { store, mac, tx }
    }

    /// Appends events and returns after the batch holding them is
    /// durable. The store error text travels back to the caller; the
    /// class stays `Internal` because the chain position is unknown.
    pub async fn append(&self, events: Vec<Event>) -> Result<AuditHead> {
        if events.is_empty() {
            return Err(Error::Invalid(
                "an audit batch must not be empty".to_owned(),
            ));
        }
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Message { events, reply })
            .map_err(|_| Error::Internal(None))?;
        rx.await.map_err(|_| Error::Internal(None))?
    }

    /// The store this log writes to, for the CLI readers.
    pub fn store(&self) -> &Arc<S> {
        &self.store
    }

    pub fn mac(&self) -> &AuditChainMac {
        &self.mac
    }

    /// Verifies the chain from `from` through `to` (None = the head).
    /// Returns the first bad seq on a break.
    pub async fn verify(&self, from: u64, to: Option<u64>) -> Result<VerifyOutcome> {
        verify::verify(self.store.as_ref(), &self.mac, from, to).await
    }
}

use crate::verify::{self, VerifyOutcome};

fn to_draft(event: &Event) -> AuditDraft {
    AuditDraft {
        ts: event.ts,
        kind: event.action.as_str().to_owned(),
        event: event.to_json(),
    }
}

async fn writer(
    store: Arc<impl Store>,
    mac: AuditChainMac,
    mut rx: mpsc::UnboundedReceiver<Message>,
) {
    loop {
        let Some(first) = rx.recv().await else {
            return;
        };
        let mut drafts: Vec<AuditDraft> = first.events.iter().map(to_draft).collect();
        let mut waiters = vec![first.reply];
        let deadline = Instant::now() + BATCH_WINDOW;
        while Instant::now() < deadline {
            match rx.try_recv() {
                Ok(next) => {
                    drafts.extend(next.events.iter().map(to_draft));
                    waiters.push(next.reply);
                }
                Err(_) if rx.is_empty() => break,
                Err(_) => tokio::time::sleep(Duration::from_millis(1)).await,
            }
        }

        let result = store.audit_append(drafts, &mac).await;
        for waiter in waiters {
            let _ = waiter.send(match &result {
                Ok(head) => Ok(head.clone()),
                Err(e) => Err(Error::Internal(Some(Box::new(CloneError(e.to_string()))))),
            });
        }
    }
}

/// The store error carries no cloneable payload, so the caller gets a
/// class-preserving copy of its display text.
#[derive(Debug)]
struct CloneError(String);

impl std::fmt::Display for CloneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CloneError {}

/// Reads the whole chain in pages and returns every row. For verify
/// and the CLI readers.
pub(crate) async fn read_all<S: Store>(
    store: &S,
    from: u64,
    limit: Option<u64>,
) -> Result<Vec<AuditRow>> {
    let mut rows = Vec::new();
    let mut next = from;
    loop {
        let page = store
            .audit_read(
                next,
                READ_PAGE,
                &riehub_store::audit::AuditFilter::default(),
            )
            .await?;
        let page_len = page.len() as u64;
        rows.extend(page);
        if page_len < READ_PAGE as u64 {
            break;
        }
        next = rows.last().map(|r| r.seq + 1).unwrap_or(next);
        if let Some(max) = limit {
            if rows.len() as u64 >= max {
                break;
            }
        }
    }
    if let Some(max) = limit {
        rows.truncate(max as usize);
    }
    Ok(rows)
}

/// Recomputes the chain over the given rows. `prev` starts empty, or
/// at the mac the first row's prune event carries after a prefix
/// prune (ADR 0010: retention deletes whole prefixes and the prune
/// event carries the last removed seq and mac).
pub(crate) fn check_chain(
    rows: &[AuditRow],
    expected_first: u64,
    mac: &AuditChainMac,
) -> std::result::Result<(), (u64, &'static str)> {
    let mut prev: Vec<u8> = Vec::new();
    let mut expected_seq = expected_first;
    let mut first = true;
    for row in rows {
        if row.seq != expected_seq {
            if first && row.seq > expected_seq {
                let carried = resume_prev(row);
                match carried {
                    Some(prev_mac) => prev = prev_mac,
                    None => return Err((expected_seq, "a row is missing before this one")),
                }
            } else {
                return Err((expected_seq, "a sequence number is missing"));
            }
        }
        let json = match serde_json::to_string(&row.event) {
            Ok(json) => json,
            Err(_) => return Err((row.seq, "the event is not valid json")),
        };
        let input = chain_input(&prev, row.seq, row.ts, &row.kind, &json);
        let expected = mac.compute(&prev, &input);
        if row.mac != expected {
            return Err((row.seq, "the mac does not match the event"));
        }
        prev = row.mac.clone();
        expected_seq = row.seq + 1;
        first = false;
    }
    Ok(())
}

fn resume_prev(row: &AuditRow) -> Option<Vec<u8>> {
    if row.kind != "audit.prune" {
        return None;
    }
    let last_mac = row.event.get("last_mac")?;
    let bytes: Vec<u64> = serde_json::from_value(last_mac.clone()).ok()?;
    if bytes.len() != 48 {
        return None;
    }
    Some(bytes.into_iter().map(|b| b as u8).collect())
}
