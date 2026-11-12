//! The audit chain (ADR 0010). Events are typed enum JSON without
//! content: ids, enums, numbers, booleans and timestamps only.

use serde_json::Value;

/// One event to append. `kind` is the variant name of the audit enum.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditDraft {
    pub ts: i64,
    pub kind: String,
    pub event: Value,
}

/// One stored event with its chain position and mac.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditRow {
    pub seq: u64,
    pub ts: i64,
    pub kind: String,
    pub event: Value,
    pub mac: Vec<u8>,
}

/// The chain head after an append.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditHead {
    pub seq: u64,
    pub mac: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuditFilter {
    pub kind: Option<String>,
    pub from_ts: Option<i64>,
    pub to_ts: Option<i64>,
}

/// Computes the chain mac. The implementation comes from
/// `riehub-crypto` at the call site: `HMAC-SHA384(audit_key,
/// prev_mac || event)` (ADR 0010). The key lives in the instance
/// coffer, never in the database.
pub trait ChainMac: Send + Sync {
    fn compute(&self, prev_mac: &[u8], event: &[u8]) -> [u8; 48];
}

/// The bytes one audit row's mac covers: prev mac, then the row body.
pub(crate) fn mac_input(
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
