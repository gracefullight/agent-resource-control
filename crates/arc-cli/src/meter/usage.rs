//! `arcctl meter summary --by vendor|project|period --json` (lane B, MTR-004,
//! MTR-018). One bounded authenticated `meter.usage_summary` read; never
//! launches work or fabricates an aggregate. The daemon's reply is checked
//! against the request (schema, echoed range and bucket, size) and emitted as
//! one JSON object; an unknown value stays `null` with its reason.
use arc_ipc::{
    client::{exchange_until, ExchangeOptions, Failure},
    meter::usage::{UsageQuery, UsageSummary, MAX_USAGE_SUMMARY_BYTES, OPERATION_USAGE_SUMMARY},
    Request,
};
use std::{
    io::{self, Write},
    path::Path,
    time::{Duration, Instant},
};

pub(super) fn run(socket: &Path, query: UsageQuery) -> Result<(), String> {
    let mut request = Request::new(crate::request_id(), OPERATION_USAGE_SUMMARY);
    request.payload = serde_json::to_value(query).map_err(|_| INVALID_REPLY.to_owned())?;
    let value = exchange_until(
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
    .map_err(|error| failure(error).to_owned())?;
    let bytes = render(&value, &query).map_err(str::to_owned)?;
    let stdout = io::stdout();
    let mut output = stdout.lock();
    output
        .write_all(&bytes)
        .and_then(|()| output.write_all(b"\n"))
        .map_err(|_| "IPC_ERROR\nCould not write the usage summary.".to_owned())
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

fn failure(error: Failure) -> &'static str {
    match error {
        Failure::Unsupported => "UNSUPPORTED_CAPABILITY\nThis daemon does not support meter.usage_summary. Use a daemon with usage summary support.",
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
