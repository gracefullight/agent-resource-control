//! The footprint merge of `arcctl meter summary --job <ID> --footprint
//! --json` (lane B, AT-MTR-02, G-01 item 4). Without `--footprint` the job
//! report keeps its bytes.
//!
//! With the daemon holding `arc_ipc::meter::footprint::ADVERTISEMENT`, the
//! reply of `meter.job_footprint` is added under the one top-level key
//! `arc_ipc::meter::footprint::SUMMARY_KEY` beside the v1 summary and its
//! `process_tree`, so Current Footprint and its delta are separate from the
//! cumulative Disk Written. Without the advertisement held by the very daemon
//! this connection authenticated, no request is sent; that, or a failed
//! read, still adds the key, with every reading unavailable and its reason
//! (`not_supported` or `read_failed`), so an explicitly requested footprint
//! is never silently absent or zero.
use arc_ipc::{
    client::{Continuity, Session},
    meter::{
        footprint::{
            JobFootprintReply, JobFootprintRequest, ADVERTISEMENT, BASIS_ALLOCATED,
            MAX_JOB_FOOTPRINT_BYTES, OPERATION_JOB_FOOTPRINT, SUMMARY_KEY,
        },
        quality::Flagged,
    },
    Request,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub const REASON_NOT_SUPPORTED: &str = "not_supported";
pub const REASON_READ_FAILED: &str = "read_failed";

pub(super) fn extend(socket: &Path, job: &str, summary: Vec<u8>) -> Result<Vec<u8>, String> {
    let footprint = read(socket, job).unwrap_or_else(unavailable);
    Ok(merge(&summary, footprint).unwrap_or(summary))
}

/// Every reading unavailable for `reason`.
fn unavailable(reason: &'static str) -> serde_json::Value {
    let missing = Flagged::<u64>::unavailable(BASIS_ALLOCATED, reason);
    serde_json::json!({
        "start_bytes": missing,
        "end_bytes": missing,
        "delta_bytes": missing,
    })
}

/// The `footprint` value from the advertising daemon, or why there is none.
fn read(socket: &Path, job: &str) -> Result<serde_json::Value, &'static str> {
    let job = arc_ipc::jobs::JobId::try_from(job.to_owned()).map_err(|_| REASON_READ_FAILED)?;
    let continuity = Continuity::default();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut session = Session::connect_until(socket, deadline, &|| false, Some(&continuity))
        .map_err(|_| REASON_READ_FAILED)?;
    let daemon = continuity.identity().ok_or(REASON_READ_FAILED)?.pid;
    if super::usage::advertiser(socket, ADVERTISEMENT) != Some(daemon) {
        return Err(REASON_NOT_SUPPORTED);
    }
    read_from(&mut session, &job).ok_or(REASON_READ_FAILED)
}

fn read_from(session: &mut Session, job: &arc_ipc::jobs::JobId) -> Option<serde_json::Value> {
    let mut request = Request::new(crate::request_id(), OPERATION_JOB_FOOTPRINT);
    request.payload = serde_json::to_value(JobFootprintRequest {
        job_id: job.clone(),
    })
    .ok()?;
    let value = session.exchange(&request, &|| false).ok()?;
    decode(&value, job)
}

fn decode(value: &serde_json::Value, job: &arc_ipc::jobs::JobId) -> Option<serde_json::Value> {
    if value.to_string().len() > MAX_JOB_FOOTPRINT_BYTES {
        return None;
    }
    let reply: JobFootprintReply = serde_json::from_value(value.clone()).ok()?;
    reply
        .answers(job)
        .then(|| serde_json::to_value(reply.footprint).ok())
        .flatten()
}

fn merge(summary: &[u8], footprint: serde_json::Value) -> Option<Vec<u8>> {
    let mut document: serde_json::Value = serde_json::from_slice(summary).ok()?;
    let object = document.as_object_mut()?;
    if object.contains_key(SUMMARY_KEY) {
        return None;
    }
    object.insert(SUMMARY_KEY.to_owned(), footprint);
    serde_json::to_vec(&document).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_a_reply_for_this_job_is_merged_under_its_own_key() {
        let job = "a".repeat(32);
        let id = arc_ipc::jobs::JobId::try_from(job.clone()).unwrap();
        let missing = json!({"value":null,"availability":"unavailable","basis":"allocated","reason":"not_measured"});
        let footprint = json!({"start_bytes":missing,"end_bytes":missing,"delta_bytes":missing});
        let reply = json!({"schema_version":1,"job_id":job,"footprint":footprint});
        assert_eq!(decode(&reply, &id), Some(footprint.clone()));
        let other = arc_ipc::jobs::JobId::try_from("b".repeat(32)).unwrap();
        assert_eq!(decode(&reply, &other), None);
        let merged = merge(br#"{"job_id":"x"}"#, footprint.clone()).unwrap();
        let merged: serde_json::Value = serde_json::from_slice(&merged).unwrap();
        assert_eq!(merged[SUMMARY_KEY], footprint);
        assert_eq!(merge(br#"{"footprint":1}"#, footprint), None);
    }

    #[test]
    fn a_requested_footprint_without_a_reading_is_unavailable_with_its_reason() {
        let value = unavailable(REASON_NOT_SUPPORTED);
        for key in ["start_bytes", "end_bytes", "delta_bytes"] {
            let reading: Flagged<i64> = serde_json::from_value(value[key].clone()).unwrap();
            assert!(reading.is_consistent() && reading.value.is_none(), "{key}");
            assert_eq!(reading.reason.as_deref(), Some(REASON_NOT_SUPPORTED));
            assert_eq!(reading.basis, BASIS_ALLOCATED);
        }
        // No daemon at all: the key is still added, and says why.
        let socket = std::path::Path::new("/nonexistent/arc-footprint-test.sock");
        let extended = extend(socket, &"a".repeat(32), br#"{"job_id":"x"}"#.to_vec()).unwrap();
        let extended: serde_json::Value = serde_json::from_slice(&extended).unwrap();
        assert_eq!(
            extended[SUMMARY_KEY]["end_bytes"]["reason"],
            REASON_READ_FAILED
        );
    }
}
