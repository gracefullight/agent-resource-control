use super::{
    apply_observation, complete, get, observe, recent, start, JobError, ReconcileResult,
    StartEvidence,
};
use arc_core::process_identity::{IdentityStatus, ProcessIdentity};
use arc_ipc::jobs::{
    CompleteRequest, JobId, JobSnapshot, Outcome, Phase, StartDisposition, StartRequest,
    StatusResult,
};
use rusqlite::Connection;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
mod migration;
mod recovery;
mod regression;
mod terminal_outcomes;
mod transitions;

struct Database(PathBuf);
impl Database {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "arc-lifecycle-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir.join("test.sqlite3"))
    }
    fn open(&self) -> Connection {
        crate::open_and_migrate(&self.0).unwrap()
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        fs::remove_dir_all(self.0.parent().unwrap()).unwrap();
    }
}
fn id(n: u32) -> JobId {
    JobId::try_from(format!("{n:032x}")).unwrap()
}
fn observer() -> ProcessIdentity {
    ProcessIdentity::capture(std::process::id()).unwrap()
}
fn register(n: u32) -> StartRequest {
    StartRequest::Register {
        job_id: id(n),
        observer: observer(),
        classification: arc_detector_classification(),
    }
}
fn arc_detector_classification() -> arc_ipc::Classification {
    serde_json::from_str(r#"{"classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNSUPPORTED_SYNTAX","uncertainty":"OBSERVE"}"#).unwrap()
}
fn mutate(c: &mut Connection, request: &StartRequest, now: u64) -> arc_ipc::jobs::StartResult {
    start(c, request, &StartEvidence::gather(request), now).unwrap()
}
fn claimed(c: &mut Connection, n: u32) -> JobSnapshot {
    let r = register(n);
    mutate(c, &r, 100);
    mutate(
        c,
        &StartRequest::Claim {
            job_id: id(n),
            observer: observer(),
        },
        101,
    )
    .job
}
fn completion(n: u32, outcome: Outcome) -> CompleteRequest {
    CompleteRequest {
        job_id: id(n),
        observer: observer(),
        outcome,
    }
}
