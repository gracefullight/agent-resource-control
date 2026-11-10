use super::*;
use crate::jobs::{get, JobError};
use arc_core::process_identity::{IdentityStatus, ProcessIdentity};
use arc_ipc::{
    jobs::{JobId, Outcome, SpawnReason, StartRequest},
    traffic::{
        BypassReason, JobRequest as Request, JobResult as Reply, LineageContext, StatusResult,
        WaitReason,
    },
    Classification,
};
use arc_traffic::Policy;
use rusqlite::Connection;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Database(PathBuf);
impl Database {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "arc-traffic-storage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir.join("db.sqlite3"))
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
fn context(n: u32) -> LineageContext {
    LineageContext::try_from(format!("{n:064x}")).unwrap()
}
fn identity() -> ProcessIdentity {
    ProcessIdentity::capture(std::process::id()).unwrap()
}
fn classification(class: &str) -> Classification {
    let (locality, reason, uncertainty) = match class {
        "BUILD" => ("HOST_LOCAL", "CARGO_BUILD", "NONE"),
        "TEST" => ("HOST_LOCAL", "CARGO_TEST", "NONE"),
        "LINT" => ("HOST_LOCAL", "CARGO_CLIPPY", "NONE"),
        "REMOTE_EXEC" => ("REMOTE", "SSH_REMOTE_COMMAND", "NONE"),
        "UNKNOWN" => ("UNKNOWN", "UNSUPPORTED_SYNTAX", "OBSERVE"),
        _ => unreachable!(),
    };
    serde_json::from_value(serde_json::json!({"classification": class, "locality": locality, "reason": reason, "uncertainty": uncertainty})).unwrap()
}
fn request(n: u32, class: &str, inherited: Option<LineageContext>) -> Request {
    Request::Enqueue {
        job_id: id(n),
        observer: identity(),
        classification: classification(class),
        lineage_context: inherited,
    }
}
fn add(c: &mut Connection, n: u32, class: &str, now: u64, policy: &Policy) -> Reply {
    let r = request(n, class, None);
    enqueue(
        c,
        &r,
        &OwnerEvidence::gather(&identity()),
        policy,
        &context(n),
        None,
        now,
    )
    .unwrap()
}
fn poll_job(c: &mut Connection, n: u32, now: u64, policy: &Policy) -> Reply {
    poll(
        c,
        &Request::Poll {
            job_id: id(n),
            observer: identity(),
        },
        &OwnerEvidence::gather(&identity()),
        policy,
        &context(n),
        now,
    )
    .unwrap()
}
fn complete_job(c: &mut Connection, n: u32, generation: u64, outcome: Outcome, now: u64) -> Reply {
    complete(
        c,
        &Request::Complete {
            job_id: id(n),
            observer: identity(),
            outcome,
            permit_generation: generation,
        },
        now,
    )
    .unwrap()
}
fn cancel_job(c: &mut Connection, n: u32, generation: Option<u64>, now: u64) -> Reply {
    cancel(
        c,
        &Request::Cancel {
            job_id: id(n),
            observer: identity(),
            permit_generation: generation,
        },
        now,
    )
    .unwrap()
}
fn fallback_request(
    n: u32,
    generation: Option<u64>,
    child: Option<ProcessIdentity>,
    outcome: Option<Outcome>,
) -> Request {
    Request::Fallback {
        job_id: id(n),
        observer: identity(),
        permit_generation: generation,
        child,
        outcome,
    }
}
fn permit_counts(c: &Connection) -> (i64, i64) {
    c.query_row(
        "SELECT COUNT(*),COALESCE(SUM(CASE WHEN state='ACTIVE' THEN 1 ELSE 0 END),0) FROM permits",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap()
}
fn generation(reply: &Reply) -> u64 {
    match reply {
        Reply::Granted {
            permit_generation, ..
        }
        | Reply::Reused {
            permit_generation, ..
        } => *permit_generation,
        other => panic!("expected grant, got {other:?}"),
    }
}

mod admission;
mod identity;
mod lineage;
mod recovery;
mod recovery_2;
mod recovery_3;
mod uncertain;
mod witness;
