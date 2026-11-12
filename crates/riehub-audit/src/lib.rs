//! riehub-audit: typed events, hash chain, verification (ADR 0010).

#![forbid(unsafe_code)]

pub mod event;
pub mod log;
pub mod verify;

pub use event::{Actor, AuditAction, AuditEvent, Event, IpField, Mac, Outcome, Target};
pub use log::{AuditLog, KeySource};
pub use verify::VerifyOutcome;

use riehub_crypto::Key32;

/// The mac the chain uses: `HMAC-SHA384(audit_key, prev_mac || bytes)`
/// (ADR 0010). The audit key lives in the instance coffer, never in
/// the database. The key sits behind an `Arc` because `Key32` never
/// clones its bytes.
pub struct AuditChainMac {
    key: std::sync::Arc<Key32>,
}

impl Clone for AuditChainMac {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
        }
    }
}

impl std::fmt::Debug for AuditChainMac {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[audit-mac]")
    }
}

impl AuditChainMac {
    pub fn new(key: Key32) -> Self {
        Self {
            key: std::sync::Arc::new(key),
        }
    }

    pub fn key(&self) -> &Key32 {
        &self.key
    }
}

impl riehub_store::audit::ChainMac for AuditChainMac {
    fn compute(&self, prev_mac: &[u8], input: &[u8]) -> [u8; 48] {
        let mut data = Vec::with_capacity(prev_mac.len() + input.len());
        data.extend_from_slice(prev_mac);
        data.extend_from_slice(input);
        riehub_crypto::hmac(&self.key, &data)
    }
}

/// The bytes one row's mac covers: prev mac, then seq, ts, kind and
/// the event body. This must mirror the layout riehub-store feeds to
/// `ChainMac::compute`, so verification recomputes exactly what
/// append stored.
pub(crate) fn chain_input(
    prev_mac: &[u8],
    seq: u64,
    ts: i64,
    kind: &str,
    event_json: &str,
) -> Vec<u8> {
    let mut input = Vec::with_capacity(prev_mac.len() + kind.len() + event_json.len() + 16);
    input.extend_from_slice(prev_mac);
    input.extend_from_slice(&seq.to_le_bytes());
    input.extend_from_slice(&ts.to_le_bytes());
    input.extend_from_slice(kind.as_bytes());
    input.extend_from_slice(event_json.as_bytes());
    input
}
