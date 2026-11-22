//! The process-tree merge of `arcctl meter summary --job --json` (lane A).
//!
//! With the daemon holding `arc_ipc::meter::job_tree::ADVERTISEMENT`, the
//! reply of `meter.job_tree` is added under the one top-level key
//! `arc_ipc::meter::job_tree::SUMMARY_KEY` beside the unchanged v1 summary:
//! the report, or `null` when none is recorded. Without the advertisement
//! held by the very daemon this connection authenticated, no request is sent
//! and the v1 bytes are returned unchanged; so is a failed optional read,
//! which never fabricates an absent tree.
use arc_ipc::{
    client::{Continuity, Session},
    meter::job_tree::{
        decode_reply, JobTreeRequest, ADVERTISEMENT, OPERATION_JOB_TREE, SUMMARY_KEY,
    },
    Request,
};
use std::{
    fs::OpenOptions,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::Path,
    time::{Duration, Instant},
};

pub(super) fn extend(socket: &Path, job: &str, summary: Vec<u8>) -> Result<Vec<u8>, String> {
    Ok(read(socket, job)
        .and_then(|tree| merge(&summary, tree))
        .unwrap_or(summary))
}

/// The `process_tree` value from the advertising daemon, if it answered.
fn read(socket: &Path, job: &str) -> Option<serde_json::Value> {
    let job = arc_ipc::jobs::JobId::try_from(job.to_owned()).ok()?;
    let continuity = Continuity::default();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut session =
        Session::connect_until(socket, deadline, &|| false, Some(&continuity)).ok()?;
    if !holds(socket, continuity.identity()?.pid) {
        return None;
    }
    let mut request = Request::new(crate::request_id(), OPERATION_JOB_TREE);
    request.payload = serde_json::json!(JobTreeRequest {
        job_id: job.clone()
    });
    let value = session.exchange(&request, &|| false).ok()?;
    let reply = decode_reply(&value, &job).ok()?;
    serde_json::to_value(reply.process_tree).ok()
}

fn merge(summary: &[u8], tree: serde_json::Value) -> Option<Vec<u8>> {
    let mut document: serde_json::Value = serde_json::from_slice(summary).ok()?;
    let object = document.as_object_mut()?;
    if object.contains_key(SUMMARY_KEY) {
        return None;
    }
    object.insert(SUMMARY_KEY.to_owned(), tree);
    serde_json::to_vec(&document).ok()
}

/// Whether process `daemon` holds the write lock on the advertisement beside
/// `socket`; the kernel names the holder, so a stale file advertises nothing.
fn holds(socket: &Path, daemon: u32) -> bool {
    let Some(directory) = socket.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return false;
    };
    let Ok(file) = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(directory.join(ADVERTISEMENT))
    else {
        return false;
    };
    // SAFETY: geteuid has no preconditions.
    if !file
        .metadata()
        .is_ok_and(|m| m.is_file() && m.uid() == unsafe { libc::geteuid() })
    {
        return false;
    }
    // SAFETY: an all-zero flock is valid; fcntl receives a live descriptor
    // and writable storage. F_GETLK takes no lock of its own.
    unsafe {
        let mut lock: libc::flock = std::mem::zeroed();
        lock.l_type = libc::F_RDLCK as _;
        lock.l_whence = libc::SEEK_SET as _;
        libc::fcntl(file.as_raw_fd(), libc::F_GETLK, &mut lock) == 0
            && lock.l_type != libc::F_UNLCK as libc::c_short
            && u32::try_from(lock.l_pid).is_ok_and(|holder| holder == daemon)
    }
}

#[cfg(test)]
#[path = "job_tree/tests.rs"]
mod tests;
