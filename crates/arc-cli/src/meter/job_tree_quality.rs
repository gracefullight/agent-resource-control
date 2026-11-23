//! Optional per-value quality read; old peers keep the existing job bytes.
use arc_ipc::{
    client::{Continuity, Session},
    meter::{job_tree::JobTreeRequest, job_tree_quality as wire},
    Request,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

const FAILED: &str = "METER_UNAVAILABLE\nThe requested process-tree quality read failed. Check the daemon connection and retry.";

/// With the advertisement held by the authenticated daemon, both the
/// `process_tree` report and its `process_tree_quality` flags from the one
/// reply (one snapshot, so a sample published meanwhile cannot split them);
/// `None` without it, for the caller's unchanged v1 fallback.
pub(super) fn extend(socket: &Path, job: &str, summary: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let Some(reply) = read(socket, job)? else {
        return Ok(None);
    };
    let mut value: serde_json::Value = serde_json::from_slice(summary).map_err(|_| FAILED)?;
    let object = value.as_object_mut().ok_or(FAILED)?;
    for (key, section) in [
        (
            arc_ipc::meter::job_tree::SUMMARY_KEY,
            serde_json::to_value(&reply.process_tree),
        ),
        (
            wire::SUMMARY_KEY,
            serde_json::to_value(&reply.process_tree_quality),
        ),
    ] {
        if object.contains_key(key) {
            return Err(FAILED.to_owned());
        }
        object.insert(key.to_owned(), section.map_err(|_| FAILED)?);
    }
    serde_json::to_vec(&value)
        .map(Some)
        .map_err(|_| FAILED.to_owned())
}

fn read(socket: &Path, job: &str) -> Result<Option<wire::Reply>, String> {
    let job = arc_ipc::jobs::JobId::try_from(job.to_owned()).map_err(|_| FAILED)?;
    let continuity = Continuity::default();
    let mut session = Session::connect_until(
        socket,
        Instant::now() + Duration::from_secs(2),
        &|| false,
        Some(&continuity),
    )
    .map_err(|_| FAILED)?;
    let daemon = continuity.identity().ok_or(FAILED)?.pid;
    if super::usage::advertiser(socket, wire::ADVERTISEMENT) != Some(daemon) {
        return Ok(None);
    }
    let mut request = Request::new(crate::request_id(), wire::OPERATION);
    request.payload = serde_json::to_value(JobTreeRequest {
        job_id: job.clone(),
    })
    .map_err(|_| FAILED)?;
    let value = session.exchange(&request, &|| false).map_err(|_| FAILED)?;
    if value.to_string().len() > wire::MAX_BYTES {
        return Err(FAILED.to_owned());
    }
    let reply: wire::Reply = serde_json::from_value(value).map_err(|_| FAILED)?;
    if !reply.answers(&job) {
        return Err(FAILED.to_owned());
    }
    Ok(Some(reply))
}
