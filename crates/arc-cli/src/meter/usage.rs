//! `arcctl meter summary --by vendor|project|period|session --json` (lane B,
//! MTR-004, MTR-018; lane A attribution section). One bounded authenticated
//! read; never launches work or fabricates an aggregate.
//!
//! A daemon holding `meter::usage::ADVERTISEMENT_V2` is asked
//! `meter.usage_summary.v2` and its reply is emitted: v2 groups (with
//! `--by session`) and, when that daemon also holds
//! `meter::attribution::ADVERTISEMENT`, the `attribution` section. Any other
//! daemon is asked the unchanged v1 `meter.usage_summary`, whose reply is
//! emitted as before; `--by session` then fails as unsupported. The reply is
//! checked against the request (schema, echoed range and bucket, size) and
//! emitted as one JSON object; an unknown value stays `null` with its reason.
use arc_ipc::{
    client::{exchange_until, ExchangeOptions, Failure},
    meter::usage::{
        UsageQuery, UsageQueryV2, UsageSummary, UsageSummaryV2, ADVERTISEMENT_V2,
        MAX_USAGE_SUMMARY_BYTES, OPERATION_USAGE_SUMMARY, OPERATION_USAGE_SUMMARY_V2,
        SCHEMA_VERSION,
    },
    Request,
};
use std::{
    io::{self, Write},
    path::Path,
    time::{Duration, Instant},
};

pub(super) fn run(socket: &Path, query: UsageQueryV2) -> Result<(), String> {
    let v2 = advertiser(socket, ADVERTISEMENT_V2).is_some();
    let bytes = match (v2, query.v1()) {
        (true, v1) => match exchange(socket, OPERATION_USAGE_SUMMARY_V2, &query) {
            Ok(value) => render_v2(&value, &query).map_err(str::to_owned)?,
            // A daemon replaced since its advertisement was read.
            Err(Failure::Unsupported) if v1.is_some() => v1_bytes(socket, v1.unwrap())?,
            Err(error) => return Err(failure(error).to_owned()),
        },
        (false, Some(v1)) => v1_bytes(socket, v1)?,
        (false, None) => return Err(failure(Failure::Unsupported).to_owned()),
    };
    let stdout = io::stdout();
    let mut output = stdout.lock();
    output
        .write_all(&bytes)
        .and_then(|()| output.write_all(b"\n"))
        .map_err(|_| "IPC_ERROR\nCould not write the usage summary.".to_owned())
}

fn exchange(
    socket: &Path,
    operation: &str,
    query: &impl serde::Serialize,
) -> Result<serde_json::Value, Failure> {
    let mut request = Request::new(crate::request_id(), operation);
    request.payload = serde_json::to_value(query).map_err(|_| Failure::InvalidReply)?;
    exchange_until(
        socket,
        &request,
        Instant::now() + Duration::from_secs(5),
        &|| false,
        &mut false,
        ExchangeOptions {
            continuity: None,
            retry_busy: false,
        },
    )
}

fn v1_bytes(socket: &Path, query: UsageQuery) -> Result<Vec<u8>, String> {
    let value = exchange(socket, OPERATION_USAGE_SUMMARY, &query)
        .map_err(|error| failure(error).to_owned())?;
    render(&value, &query).map_err(str::to_owned)
}

const INVALID_REPLY: &str = "IPC_ERROR\nThe daemon did not return a valid usage summary.";

fn render(value: &serde_json::Value, query: &UsageQuery) -> Result<Vec<u8>, &'static str> {
    let summary: UsageSummary = serde_json::from_value(value.clone()).map_err(|_| INVALID_REPLY)?;
    if summary.schema_version != arc_ipc::meter::usage::SCHEMA_VERSION
        || summary.query != *query
        || !summary.groups.iter().all(serde_json::Value::is_object)
    {
        return Err(INVALID_REPLY);
    }
    let bytes = serde_json::to_vec(&summary).map_err(|_| INVALID_REPLY)?;
    if bytes.len() > MAX_USAGE_SUMMARY_BYTES {
        return Err(INVALID_REPLY);
    }
    Ok(bytes)
}

fn render_v2(value: &serde_json::Value, query: &UsageQueryV2) -> Result<Vec<u8>, &'static str> {
    let summary: UsageSummaryV2 =
        serde_json::from_value(value.clone()).map_err(|_| INVALID_REPLY)?;
    if summary.schema_version != SCHEMA_VERSION
        || summary.query != *query
        || !summary.groups.iter().all(serde_json::Value::is_object)
        || summary.attribution.as_ref().is_some_and(|a| !a.is_valid())
    {
        return Err(INVALID_REPLY);
    }
    let bytes = serde_json::to_vec(&summary).map_err(|_| INVALID_REPLY)?;
    if bytes.len() > MAX_USAGE_SUMMARY_BYTES {
        return Err(INVALID_REPLY);
    }
    Ok(bytes)
}

fn failure(error: Failure) -> &'static str {
    match error {
        Failure::Unsupported => "UNSUPPORTED_CAPABILITY\nThis daemon does not support this usage summary (--by session needs meter.usage-summary.v2). Use a daemon with that support.",
        Failure::Rejected => "INVALID_REQUEST\nThe daemon refused this range. Use a narrower range with --from below --to, or --bucket day.",
        Failure::Unavailable | Failure::Uncertain | Failure::Busy => "METER_UNAVAILABLE\nUsage summaries are unavailable. Check the daemon connection and retry this read.",
        Failure::Protocol => "PROTOCOL_INCOMPATIBLE\nUse matching arcctl and daemon protocol versions.",
        Failure::InvalidEndpoint => "IPC_ERROR\nThe daemon endpoint could not be authenticated.",
        Failure::Cancelled => "CANCELLED\nThe usage summary read was cancelled.",
        _ => INVALID_REPLY,
    }
}

#[allow(clippy::duplicate_mod)] // Also included by the hook and `hook-exec`.
#[path = "usage/advertiser.rs"]
mod advertiser;
/// The process advertising `advertisement` beside `socket`.
pub(super) fn advertiser(socket: &Path, advertisement: &str) -> Option<u32> {
    advertiser::holder(socket, advertisement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_ipc::meter::usage::{Bucket, GroupBy};
    use serde_json::json;

    fn query() -> UsageQuery {
        UsageQuery {
            by: GroupBy::Vendor,
            from_ms: 1,
            to_ms: 2,
            bucket: Bucket::Hour,
        }
    }

    #[test]
    fn a_reply_is_emitted_only_for_the_query_it_answers() {
        let reply = json!({"schema_version":1,"query":query(),"generated_at_ms":3,
            "groups":[{"vendor":"none","bucket_start_ms":0}]});
        let bytes = render(&reply, &query()).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            reply
        );
        let mut other = query();
        other.bucket = Bucket::Day;
        assert_eq!(render(&reply, &other), Err(INVALID_REPLY));
        for invalid in [
            json!({"schema_version":2,"query":query(),"generated_at_ms":3,"groups":[]}),
            json!({"schema_version":1,"query":query(),"generated_at_ms":3,"groups":[1]}),
            json!({"schema_version":1,"query":query(),"generated_at_ms":3,"groups":[],"x":1}),
            json!(null),
        ] {
            assert_eq!(render(&invalid, &query()), Err(INVALID_REPLY), "{invalid}");
        }
    }

    #[test]
    fn a_v2_reply_is_emitted_only_for_its_query_and_a_valid_section() {
        use arc_ipc::meter::usage::{Bucket, GroupByV2};
        let query = UsageQueryV2 {
            by: GroupByV2::Session,
            from_ms: 1,
            to_ms: 2,
            bucket: Bucket::Hour,
        };
        let reply = json!({"schema_version":1,"query":query,"generated_at_ms":3,"groups":[]});
        assert!(render_v2(&reply, &query).is_ok());
        let mut newer = reply.clone();
        newer["schema_version"] = json!(2);
        assert_eq!(render_v2(&newer, &query), Err(INVALID_REPLY));
        let mut broken = reply;
        broken["attribution"] = json!({"window":{"from_ms":0,"to_ms":0,"scans":0,
            "first_scan_ms":null,"last_scan_ms":null,"skipped_processes":0},
            "proven":[],"estimated":[],
            "unattributed":{"value":null,"availability":"unavailable","basis":"scan_deltas","reason":"no_scan"},
            "unattributed_ratio":{"value":null,"availability":"unavailable","basis":"scan_deltas","reason":"no_scan"}});
        assert_eq!(render_v2(&broken, &query), Err(INVALID_REPLY));
        assert_eq!(query.v1(), None);
    }

    #[test]
    fn errors_are_static_and_name_a_next_step() {
        for error in [
            Failure::Unsupported,
            Failure::Rejected,
            Failure::Unavailable,
            Failure::Protocol,
            Failure::InvalidEndpoint,
            Failure::Cancelled,
            Failure::InvalidReply,
        ] {
            let message = failure(error);
            assert!(
                message.contains('\n') && !message.contains(" 0 "),
                "{message}"
            );
        }
        let directory =
            std::env::temp_dir().join(format!("arc-meter-usage-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(".lock"), b"").unwrap();
        assert_eq!(advertiser(&directory.join("arc.sock"), ".lock"), None);
        assert_eq!(advertiser(&directory.join("arc.sock"), ".missing"), None);
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
