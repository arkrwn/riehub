//! Usage rows and buckets (storage spec, section 3). Usage carries
//! metadata only, never content.

use riehub_core::Owner;

/// One billed call.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageRow {
    pub ts: i64,
    pub owner: Owner,
    pub user: Option<String>,
    pub team: Option<String>,
    pub hub_key: Option<String>,
    pub project: Option<String>,
    pub provider_kind: Option<String>,
    pub model: Option<String>,
    pub status: Option<String>,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub cost_micros: i64,
}

/// What to group by. A fixed enum set, so every backend groups the
/// same way (storage spec, section 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UsageGroup {
    Hour,
    Day,
    User,
    Team,
    HubKey,
    Project,
    ProviderKind,
    Model,
    Status,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsageQuery {
    pub owner: Owner,
    pub from_ts: i64,
    pub to_ts: i64,
    pub group_by: Vec<UsageGroup>,
    pub status: Option<String>,
    pub model: Option<String>,
    pub user: Option<String>,
}

/// One aggregated bucket. `keys` follows the order of
/// `UsageQuery.group_by`. `bucket_start` is the UTC millisecond start
/// of the hour or day when the query groups by time.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UsageBucket {
    pub bucket_start: i64,
    pub keys: Vec<String>,
    pub requests: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub cost_micros: i64,
}
