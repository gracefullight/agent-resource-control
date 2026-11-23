//! Negotiated usage summaries and the Traffic queue depth (lane B,
//! MTR-004/MTR-008/MTR-018/MET-013/MET-015/MET-017).
//!
//! Aggregates are computed by the daemon from recorded jobs over an explicit
//! range. Only `canonical_root` job Meter reports contribute CPU, so a nested
//! report is counted as a job but never summed twice. REMOTE_CI and
//! REMOTE_EXEC jobs are counted in their own group with local CPU `None` and
//! reason `not_metered_locally`. A job without a vendor groups under `none`
//! (a direct command with no agent hook), never under Unattributed.
//!
//! `meter.snapshot` keeps its v1 reply. A request payload
//! `{"include_traffic":true}` is sent only after the daemon advertised
//! [`SNAPSHOT_TRAFFIC_CAPABILITY`]; only such a reply carries `traffic`.
use crate::ERROR_INVALID_REQUEST;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAPABILITY: &str = "meter.usage-summary.v1";
pub const OPERATION_USAGE_SUMMARY: &str = "meter.usage_summary";
/// Also gates the registration usage fields of `jobs::usage::UsageFields`.
pub const ADVERTISEMENT: &str = ".meter-usage-summary-v1";
pub const SNAPSHOT_TRAFFIC_CAPABILITY: &str = "meter.snapshot-traffic.v1";
pub const SNAPSHOT_TRAFFIC_ADVERTISEMENT: &str = ".meter-snapshot-traffic-v1";
/// The reply key added beside the v1 snapshot fields when requested.
pub const SNAPSHOT_TRAFFIC_KEY: &str = "traffic";
pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_USAGE_SUMMARY_BYTES: usize = 262_144;

/// The negotiated newer summary (Phase 1 attribution cycle). v1
/// [`OPERATION_USAGE_SUMMARY`] keeps its request and reply bytes; a client
/// sends [`OPERATION_USAGE_SUMMARY_V2`] only to a daemon holding
/// [`ADVERTISEMENT_V2`]. The same advertisement gates the registration
/// `jobs::usage::UsageFields::session_key`. A v2 reply adds `--by session`,
/// per-value quality flags and corrected CPU in `groups`, and carries
/// `attribution` only while the daemon also holds
/// `meter::attribution::ADVERTISEMENT`.
///
/// The v2 reply is additive: it keeps `schema_version` [`SCHEMA_VERSION`]
/// and every v1 key, and only adds keys (`attribution`, the `session` group
/// key, per-value quality flags inside groups), so a reader of the v1
/// document keeps reading it.
pub const CAPABILITY_V2: &str = "meter.usage-summary.v2";
pub const OPERATION_USAGE_SUMMARY_V2: &str = "meter.usage_summary.v2";
pub const ADVERTISEMENT_V2: &str = ".meter-usage-summary-v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    Vendor,
    Project,
    Period,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bucket {
    Hour,
    Day,
}

/// `[from_ms, to_ms)` in Unix milliseconds, explicit on every request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageQuery {
    pub by: GroupBy,
    pub from_ms: u64,
    pub to_ms: u64,
    pub bucket: Bucket,
}
impl UsageQuery {
    pub fn is_valid(&self) -> bool {
        self.from_ms < self.to_ms && self.to_ms <= crate::jobs::MAX_TIMESTAMP
    }
}

/// The reply; `groups` is defined by lane B.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageSummary {
    pub schema_version: u32,
    pub query: UsageQuery,
    pub generated_at_ms: u64,
    pub groups: Vec<Value>,
}

/// v2 grouping; `session` groups by the stored session digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupByV2 {
    Vendor,
    Project,
    Period,
    Session,
}

/// The v2 request; for vendor, project and period it serializes exactly as
/// the v1 [`UsageQuery`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageQueryV2 {
    pub by: GroupByV2,
    pub from_ms: u64,
    pub to_ms: u64,
    pub bucket: Bucket,
}
impl UsageQueryV2 {
    pub fn is_valid(&self) -> bool {
        self.from_ms < self.to_ms && self.to_ms <= crate::jobs::MAX_TIMESTAMP
    }
    /// The v1 query of the same range, when the grouping exists in v1.
    pub fn v1(&self) -> Option<UsageQuery> {
        let by = match self.by {
            GroupByV2::Vendor => GroupBy::Vendor,
            GroupByV2::Project => GroupBy::Project,
            GroupByV2::Period => GroupBy::Period,
            GroupByV2::Session => return None,
        };
        Some(UsageQuery {
            by,
            from_ms: self.from_ms,
            to_ms: self.to_ms,
            bucket: self.bucket,
        })
    }
}

/// The v2 reply. `groups` is defined by lane B; `attribution` by lane A.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageSummaryV2 {
    pub schema_version: u32,
    pub query: UsageQueryV2,
    pub generated_at_ms: u64,
    pub groups: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<super::attribution::AttributionSection>,
}

pub fn decode_query_v2(payload: &Value) -> Result<UsageQueryV2, &'static str> {
    if !payload.is_object() || payload.to_string().len() > 512 {
        return Err(ERROR_INVALID_REQUEST);
    }
    let query = UsageQueryV2::deserialize(payload).map_err(|_| ERROR_INVALID_REQUEST)?;
    query
        .is_valid()
        .then_some(query)
        .ok_or(ERROR_INVALID_REQUEST)
}

/// The optional `meter.snapshot` request field.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotRequest {
    pub include_traffic: bool,
}

/// `traffic.queue.depth` from daemon state at `observed_at_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotTraffic {
    pub queue_depth: u64,
    pub observed_at_ms: u64,
}

/// `null` (protocol 1) or `{"include_traffic":bool}`; anything else is invalid.
pub fn decode_snapshot_request(payload: &Value) -> Result<SnapshotRequest, &'static str> {
    if payload.is_null() {
        return Ok(SnapshotRequest::default());
    }
    if !payload.is_object() || payload.to_string().len() > 128 {
        return Err(ERROR_INVALID_REQUEST);
    }
    SnapshotRequest::deserialize(payload).map_err(|_| ERROR_INVALID_REQUEST)
}

pub fn decode_query(payload: &Value) -> Result<UsageQuery, &'static str> {
    if !payload.is_object() || payload.to_string().len() > 512 {
        return Err(ERROR_INVALID_REQUEST);
    }
    let query = UsageQuery::deserialize(payload).map_err(|_| ERROR_INVALID_REQUEST)?;
    query
        .is_valid()
        .then_some(query)
        .ok_or(ERROR_INVALID_REQUEST)
}
