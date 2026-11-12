//! Chain verification (ADR 0010): walk the rows, recompute the macs,
//! report the first bad seq.

use crate::log::{check_chain, read_all};
use crate::AuditChainMac;
use riehub_core::error::Result;
use riehub_store::Store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyOutcome {
    /// The chain holds through `head_seq` (0 on an empty log).
    Ok { head_seq: u64 },
    /// The first bad position and why.
    Broken { at: u64, why: &'static str },
}

impl VerifyOutcome {
    pub fn is_ok(&self) -> bool {
        matches!(self, VerifyOutcome::Ok { .. })
    }
}

/// Verifies the chain from `from` through `to` (None = the head).
pub async fn verify<S: Store>(
    store: &S,
    mac: &AuditChainMac,
    from: u64,
    to: Option<u64>,
) -> Result<VerifyOutcome> {
    let limit = match to {
        Some(to) if to + 1 > from => Some(to - from + 1),
        Some(_) => Some(0),
        None => None,
    };
    let rows = read_all(store, from, limit).await?;
    match check_chain(&rows, from, mac) {
        Ok(()) => {
            let head_seq = rows.last().map(|r| r.seq).unwrap_or(0);
            Ok(VerifyOutcome::Ok { head_seq })
        }
        Err((at, why)) => Ok(VerifyOutcome::Broken { at, why }),
    }
}
