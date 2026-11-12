//! Meter-only daemon adapters. No resource history or admission decisions.
use arc_core::process_identity::ProcessIdentity;
use arc_ipc::{
    meter::{decode_request, snapshot_response, ERROR_METER_UNAVAILABLE},
    Request, Response,
};
use arc_meter::{MeterWorker, SamplingPolicy, SnapshotReader};
#[cfg(test)]
#[path = "meter_failure_fixture.rs"]
mod failure_fixture;

pub(crate) fn start() -> Option<MeterWorker> {
    let identity = ProcessIdentity::capture(std::process::id()).ok()?;
    let epoch = std::time::Instant::now();
    // Measurement/identity/thread failures must leave execution and admission
    // working. A missing sampler yields an explicit METER_UNAVAILABLE reply.
    MeterWorker::start(
        arc_meter_source(epoch),
        identity,
        epoch,
        SamplingPolicy::default(),
    )
    .ok()
}

fn arc_meter_source(epoch: std::time::Instant) -> impl arc_meter::NativeSource {
    #[cfg(test)]
    {
        failure_fixture::Source::new(epoch)
    }
    #[cfg(not(test))]
    {
        arc_meter::SystemSource::new(epoch)
    }
}

pub(crate) fn response(request: &Request, reader: Option<&SnapshotReader>) -> Response {
    if decode_request(&request.payload).is_err() {
        return Response::error(&request.request_id, arc_ipc::ERROR_INVALID_REQUEST);
    }
    match reader.and_then(SnapshotReader::snapshot) {
        Some(snapshot) => snapshot_response(request, &snapshot),
        None => Response::error(&request.request_id, ERROR_METER_UNAVAILABLE),
    }
}

/// Read committed current lifecycle/Traffic state through the existing single
/// SQLite owner. No native probes, job mutations or second connection. A failed
/// activity observation conservatively selects active cadence, never a permit.
pub(crate) fn active_jobs(connection: &rusqlite::Connection) -> bool {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM jobs WHERE state IN ('REGISTERED','LAUNCHING','RUNNING'))
         OR EXISTS(SELECT 1 FROM traffic_entries WHERE state IN ('QUEUED','RESERVED','RUNNING','RETAINED'))",
        [], |row| row.get(0),
    ).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_and_invalid_payload_are_distinct_without_a_database_or_native_call() {
        let mut request = arc_ipc::meter::snapshot_request("meter");
        assert_eq!(
            response(&request, None).error.unwrap().code,
            ERROR_METER_UNAVAILABLE
        );
        request.payload = serde_json::json!({"path":"/other"});
        assert_eq!(
            response(&request, None).error.unwrap().code,
            arc_ipc::ERROR_INVALID_REQUEST
        );
    }

    #[test]
    fn activity_uses_actual_committed_state_and_failed_reads_do_not_change_jobs() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        assert!(active_jobs(&connection));
        connection
            .execute_batch(
                "CREATE TABLE jobs(state TEXT); CREATE TABLE traffic_entries(state TEXT);",
            )
            .unwrap();
        assert!(!active_jobs(&connection));
        for state in ["REGISTERED", "LAUNCHING", "RUNNING"] {
            connection
                .execute("INSERT INTO jobs(state) VALUES(?1)", [state])
                .unwrap();
            assert!(active_jobs(&connection));
            connection.execute("DELETE FROM jobs", []).unwrap();
        }
        for state in ["QUEUED", "RESERVED", "RUNNING", "RETAINED"] {
            connection
                .execute("INSERT INTO traffic_entries(state) VALUES(?1)", [state])
                .unwrap();
            assert!(active_jobs(&connection));
            connection
                .execute("DELETE FROM traffic_entries", [])
                .unwrap();
        }
        for state in [
            "SUCCEEDED",
            "FAILED",
            "SIGNALED",
            "SPAWN_FAILED",
            "COMPLETED",
            "OUTCOME_UNKNOWN",
        ] {
            connection
                .execute("INSERT INTO jobs(state) VALUES(?1)", [state])
                .unwrap();
            assert!(!active_jobs(&connection));
        }
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM jobs", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            6
        );
    }
}
