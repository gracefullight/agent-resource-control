//! Every recorded decision of one job and its current queue position or
//! expected schedule (lane C, TRF-015).
//!
//! Read with `traffic.decision_history` on the same authenticated connection
//! only from a daemon holding `arc_ipc::traffic::decision_history::ADVERTISEMENT`
//! (the kernel names the lock holder, which must be the peer of this
//! session), so an older or scripted peer sees exactly the requests it saw
//! before. An older daemon, or any failure of this optional read, leaves
//! today's output unchanged.
use arc_ipc::{
    client::Session,
    jobs::JobId,
    traffic::decision_history::{
        decode_reply, DecisionHistory, HistoryRequest, ADVERTISEMENT, OPERATION_DECISION_HISTORY,
    },
    Request,
};
use serde_json::{json, Value};
use std::{
    fs::OpenOptions,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::Path,
};

/// JSON key of the history beside the v1 diagnostics fields.
pub(super) const KEY: &str = "decision_history";

pub(super) fn read(
    session: &mut Session,
    socket: &Path,
    daemon: Option<u32>,
    job: &JobId,
) -> Option<DecisionHistory> {
    if !advertised(socket, daemon?) {
        return None;
    }
    let mut request = Request::new(crate::request_id(), OPERATION_DECISION_HISTORY);
    request.payload = json!(HistoryRequest {
        job_id: job.clone()
    });
    let value = session.exchange(&request, &|| false).ok()?;
    decode_reply(&value, job).ok()
}

/// Whether process `daemon` holds the write lock on the advertisement file
/// beside `socket`; a file nobody locks advertises nothing.
fn advertised(socket: &Path, daemon: u32) -> bool {
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

fn words(reason: &str) -> &'static str {
    match reason {
        "start_rate" => " (waiting for a start-rate token)",
        "fifo" => " (waiting for its turn in the queue)",
        "class_window" => " (waiting for a class window)",
        "cpu_budget" => " (waiting for the CPU budget)",
        "memory_budget" => " (waiting for the memory budget)",
        "memory_pressure" => " (held under memory pressure)",
        _ => "",
    }
}

/// The text block after the existing lines; empty without a history.
pub(super) fn lines(history: Option<&DecisionHistory>) -> String {
    let Some(history) = history else {
        return String::new();
    };
    let mut text = String::from("Decision history:\n");
    if history.decisions.is_empty() {
        text.push_str("  none recorded\n");
    }
    for entry in &history.decisions {
        text.push_str(&format!(
            "  {}. {} {} {}{}\n",
            entry.index,
            entry.decided_at_ms,
            entry.kind,
            entry.reason,
            words(&entry.reason)
        ));
    }
    match history.queue_position {
        Some((position, of)) => {
            text.push_str(&format!("Queue position: {position} of {of}\n"));
            match history.expected_schedule_ms {
                Some(at) => text.push_str(&format!(
                    "Expected schedule: start-rate token at {at} (Unix ms)\n"
                )),
                None => text.push_str("Expected schedule: unknown\n"),
            }
        }
        None => text.push_str("Queue position: not queued\n"),
    }
    text
}

/// The JSON document; a history adds its own key beside the v1 fields.
pub(super) fn document(mut document: Value, history: Option<&DecisionHistory>) -> Value {
    if let (Some(history), Some(object)) = (history, document.as_object_mut()) {
        object.insert(KEY.to_owned(), json!(history));
    }
    document
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
