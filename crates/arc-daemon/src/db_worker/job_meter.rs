//! Authenticated terminal reports/readbacks on the existing finite DB queue.
use super::{jobs, DbClient};
use arc_core::process_identity::{probe, probe_live, IdentityStatus, ProcessIdentity};
use arc_ipc::meter::{JobMeterReport, JobMeterRequest, JobMeterSummary, ReportReceipt};
use arc_storage::job_meter::{self, LineageEvidence, MeterError};
use std::time::Instant;

pub(crate) enum Operation {
    Report(Box<JobMeterReport>),
    Summary(String),
}
pub(crate) enum Value {
    Report(Result<ReportReceipt, MeterError>),
    Summary(Result<JobMeterSummary, MeterError>),
}
pub(crate) fn execute(connection: &mut rusqlite::Connection, operation: Operation) -> Value {
    match operation {
        Operation::Report(report) => {
            Value::Report(job_meter::report(connection, &report, jobs::now()))
        }
        Operation::Summary(id) => Value::Summary(job_meter::summary(connection, &id, jobs::now())),
    }
}
pub(crate) async fn handle(
    client: DbClient,
    input: JobMeterRequest,
    peer_pid: u32,
    deadline: Instant,
) -> Result<serde_json::Value, &'static str> {
    let operation = match input {
        JobMeterRequest::Report(report) => {
            if !report.is_valid() {
                return Err(arc_ipc::ERROR_INVALID_REQUEST);
            }
            // Peer identity is native authorization; a tuple in JSON alone is
            // insufficient. Never probe the child after its terminal reap.
            if peer_pid != report.observer.pid
                || probe_live(&report.observer) != IdentityStatus::VerifiedMatch
            {
                return Err(arc_ipc::meter::ERROR_METER_IDENTITY_MISMATCH);
            }
            Operation::Report(report)
        }
        JobMeterRequest::Summary(request) => {
            if !arc_core::job_meter::valid_id(&request.job_id) {
                return Err(arc_ipc::ERROR_INVALID_REQUEST);
            }
            Operation::Summary(request.job_id)
        }
    };
    match jobs::call(&client, jobs::Operation::Meter(operation), deadline).await? {
        jobs::Value::Meter(Value::Report(value)) => {
            serde_json::to_value(value.map_err(MeterError::code)?)
                .map_err(|_| arc_ipc::ERROR_DB_BUSY)
        }
        jobs::Value::Meter(Value::Summary(value)) => {
            serde_json::to_value(value.map_err(MeterError::code)?)
                .map_err(|_| arc_ipc::ERROR_DB_BUSY)
        }
        _ => Err(arc_ipc::ERROR_DB_BUSY),
    }
}

pub(crate) fn evidence(
    observer: &ProcessIdentity,
    child: Option<&ProcessIdentity>,
) -> LineageEvidence {
    LineageEvidence::gather(observer, child, chain, owned_child)
}
pub(super) fn owned_child(child: &ProcessIdentity, observer: &ProcessIdentity) -> bool {
    child != observer
        && probe(child) == IdentityStatus::VerifiedMatch
        && parent_pid(child.pid).ok() == Some(observer.pid)
        && probe_live(observer) == IdentityStatus::VerifiedMatch
        && probe(child) == IdentityStatus::VerifiedMatch
}
fn parent_pid(pid: u32) -> std::io::Result<u32> {
    match super::traffic::parent_pid(pid) {
        Ok(parent) => Ok(parent),
        Err(error) => {
            #[cfg(target_os = "macos")]
            {
                unsafe extern "C" {
                    fn arc_unreaped_parent_pid(pid: i32, parent: *mut i32) -> i32;
                }
                let mut parent = 0;
                // SAFETY: positive validated PID and an exact writable int. The
                // C adapter uses SDK types, never a guessed Rust kernel layout.
                let code = unsafe { arc_unreaped_parent_pid(pid as i32, &mut parent) };
                if code == 0 && parent > 0 {
                    return Ok(parent as u32);
                }
            }
            Err(error)
        }
    }
}
fn chain(observer: &ProcessIdentity) -> (Vec<ProcessIdentity>, bool) {
    let mut current = observer.clone();
    let mut result = Vec::new();
    for _ in 0..128 {
        if probe_live(&current) != IdentityStatus::VerifiedMatch {
            return (result, false);
        }
        if current.pid == 1 {
            return (result, true);
        }
        let Ok(pid) = super::traffic::parent_pid(current.pid) else {
            return (result, false);
        };
        if pid == 0 || pid == current.pid {
            return (result, false);
        }
        // As in arc-platform's lineage walker, a stable native ppid of 1 is
        // the userspace boundary. launchd/init cannot be a managed wrapper;
        // reading its protected identity would incorrectly reject this root.
        if pid == 1 {
            let stable = probe_live(&current) == IdentityStatus::VerifiedMatch
                && super::traffic::parent_pid(current.pid).ok() == Some(1);
            return (result, stable);
        }
        let Ok(parent) = ProcessIdentity::capture(pid) else {
            return (result, false);
        };
        if probe_live(&current) != IdentityStatus::VerifiedMatch
            || super::traffic::parent_pid(current.pid).ok() != Some(pid)
            || probe_live(&parent) != IdentityStatus::VerifiedMatch
            || result.contains(&parent)
        {
            return (result, false);
        }
        result.push(parent.clone());
        current = parent;
    }
    (result, false)
}

#[cfg(test)]
#[path = "job_meter_tests.rs"]
mod tests;
