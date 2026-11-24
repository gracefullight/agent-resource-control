//! `arcctl meter summary --by vendor|project|period|session --json` (lane B,
//! MTR-004, MTR-018; lane A attribution section). One bounded authenticated
//! read; never launches work or fabricates an aggregate.
//!
//! A daemon holding `meter::usage::ADVERTISEMENT_V2` is asked
//! `meter.usage_summary.v2` and its reply is emitted: v2 groups (with
//! `--by session`) and, when that daemon also holds
//! `meter::attribution::ADVERTISEMENT`, the `attribution` section. Any other
//! daemon is asked the unchanged v1 `meter.usage_summary`, whose reply is
//! emitted as before; `--by session` then fails as unsupported. A daemon also
//! holding `meter::usage::CODEX_ADVERTISEMENT` is asked `meter.usage_summary.v3`
//! first, whose groups may name Codex. A replaced daemon's Unsupported answer
//! falls back through v2 to v1 in that order. The reply is
//! checked against the request (schema, echoed range and bucket, size) and
//! emitted as one JSON object; an unknown value stays `null` with its reason.
use arc_ipc::{
    client::{exchange_until, ExchangeOptions, Failure},
    meter::usage::{
        UsageQuery, UsageQueryV2, UsageSummary, UsageSummaryV2, ADVERTISEMENT_V2,
        CODEX_ADVERTISEMENT, MAX_USAGE_SUMMARY_BYTES, OPERATION_USAGE_SUMMARY,
        OPERATION_USAGE_SUMMARY_V2, OPERATION_USAGE_SUMMARY_V3, SCHEMA_VERSION,
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
    let v3 = advertiser(socket, CODEX_ADVERTISEMENT).is_some();
    let bytes = summary_bytes(v3, v2, &query, &mut |operation, payload| {
        exchange(socket, operation, &payload)
    })?;
    let stdout = io::stdout();
    let mut output = stdout.lock();
    output
        .write_all(&bytes)
        .and_then(|()| output.write_all(b"\n"))
        .map_err(|_| "IPC_ERROR\nCould not write the usage summary.".to_owned())
}

/// Ask the newest advertised summary first. A daemon replaced since its
/// advertisement was read answers Unsupported; each step then falls back to
/// the next older operation, ending at v1 when the query has that form.
fn summary_bytes(
    v3: bool,
    v2: bool,
    query: &UsageQueryV2,
    exchange: &mut dyn FnMut(&'static str, serde_json::Value) -> Result<serde_json::Value, Failure>,
) -> Result<Vec<u8>, String> {
    let payload = serde_json::to_value(query).map_err(|_| INVALID_REPLY.to_owned())?;
    let newer = [
        (v3, OPERATION_USAGE_SUMMARY_V3),
        (v2, OPERATION_USAGE_SUMMARY_V2),
    ];
    for operation in newer.into_iter().filter(|(on, _)| *on).map(|(_, op)| op) {
        match exchange(operation, payload.clone()) {
            Ok(value) => return render_v2(&value, query).map_err(str::to_owned),
            Err(Failure::Unsupported) => {}
            Err(error) => return Err(failure(error).to_owned()),
        }
    }
    let Some(v1) = query.v1() else {
        return Err(failure(Failure::Unsupported).to_owned());
    };
    let payload = serde_json::to_value(v1).map_err(|_| INVALID_REPLY.to_owned())?;
    let value = exchange(OPERATION_USAGE_SUMMARY, payload).map_err(|e| failure(e).to_owned())?;
    render(&value, &v1).map_err(str::to_owned)
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

    fn v1_reply(query: &UsageQuery) -> serde_json::Value {
        json!({"schema_version":1,"query":query,"generated_at_ms":3,
            "groups":[{"vendor":"none","bucket_start_ms":0}]})
    }

    /// v2 and v3 were advertised, then a v1-only daemon replaced the
    /// advertiser before the requests: each newer request is Unsupported and
    /// the read ends with the v1 query.
    #[test]
    fn a_replaced_daemon_falls_back_from_v3_through_v2_to_v1() {
        use arc_ipc::meter::usage::{Bucket, GroupByV2};
        let query = UsageQueryV2 {
            by: GroupByV2::Vendor,
            from_ms: 1,
            to_ms: 2,
            bucket: Bucket::Hour,
        };
        let v1 = query.v1().unwrap();
        for (v3, v2, expected) in [
            (
                true,
                true,
                &[
                    OPERATION_USAGE_SUMMARY_V3,
                    OPERATION_USAGE_SUMMARY_V2,
                    OPERATION_USAGE_SUMMARY,
                ][..],
            ),
            (
                true,
                false,
                &[OPERATION_USAGE_SUMMARY_V3, OPERATION_USAGE_SUMMARY][..],
            ),
            (
                false,
                true,
                &[OPERATION_USAGE_SUMMARY_V2, OPERATION_USAGE_SUMMARY][..],
            ),
            (false, false, &[OPERATION_USAGE_SUMMARY][..]),
        ] {
            let mut asked = Vec::new();
            let bytes = summary_bytes(v3, v2, &query, &mut |operation, payload| {
                asked.push(operation);
                if operation == OPERATION_USAGE_SUMMARY {
                    assert_eq!(payload, serde_json::to_value(v1).unwrap());
                    Ok(v1_reply(&v1))
                } else {
                    assert_eq!(payload, serde_json::to_value(query).unwrap());
                    Err(Failure::Unsupported)
                }
            })
            .unwrap();
            assert_eq!(asked, expected, "v3={v3} v2={v2}");
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
                v1_reply(&v1)
            );
        }
    }

    #[test]
    fn the_newest_answer_wins_and_other_failures_do_not_fall_back() {
        use arc_ipc::meter::usage::{Bucket, GroupByV2};
        let query = UsageQueryV2 {
            by: GroupByV2::Vendor,
            from_ms: 1,
            to_ms: 2,
            bucket: Bucket::Hour,
        };
        let reply = json!({"schema_version":1,"query":query,"generated_at_ms":3,"groups":[]});
        let mut asked = Vec::new();
        let bytes = summary_bytes(true, true, &query, &mut |operation, _| {
            asked.push(operation);
            Ok(reply.clone())
        })
        .unwrap();
        assert_eq!(asked, [OPERATION_USAGE_SUMMARY_V3]);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            reply
        );
        let mut asked = Vec::new();
        let error = summary_bytes(true, true, &query, &mut |operation, _| {
            asked.push(operation);
            Err(Failure::Rejected)
        })
        .unwrap_err();
        assert_eq!(asked, [OPERATION_USAGE_SUMMARY_V3]);
        assert_eq!(error, failure(Failure::Rejected));
        // `--by session` has no v1 form: a v1-only replacement is reported.
        let session = UsageQueryV2 {
            by: GroupByV2::Session,
            ..query
        };
        let mut asked = Vec::new();
        let error = summary_bytes(true, true, &session, &mut |operation, _| {
            asked.push(operation);
            Err(Failure::Unsupported)
        })
        .unwrap_err();
        assert_eq!(
            asked,
            [OPERATION_USAGE_SUMMARY_V3, OPERATION_USAGE_SUMMARY_V2]
        );
        assert_eq!(error, failure(Failure::Unsupported));
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
