//! Traffic resilience through built `arcctl exec` and a real daemon
//! (TRF-009, AT-TRF-10, NFR-009). Tools are offline stubs.
#[path = "traffic_resilience/attach.rs"]
mod attach;
#[allow(dead_code)] // The proxy's other modes serve managed_exec.
#[path = "managed_exec/fault.rs"]
mod fault;
#[path = "traffic_resilience/faults.rs"]
mod faults;
#[path = "traffic_resilience/fixture.rs"]
mod fixture;
#[path = "traffic_resilience/invariants.rs"]
mod invariants;
#[path = "support/journey.rs"]
mod journey;
#[allow(dead_code)] // Only the child barrier is used here.
#[path = "support/native_exec.rs"]
mod native_exec;
#[path = "traffic_resilience/protocol.rs"]
mod protocol;
#[path = "managed_exec/relay.rs"]
mod relay;
#[path = "traffic_resilience/restart.rs"]
mod restart;
#[allow(dead_code)] // The shared fixture also serves unrelated workloads.
#[path = "support/runtime.rs"]
mod runtime;
#[path = "traffic_resilience/schema_step_back.rs"]
mod schema_step_back;
mod support;

use arc_storage::{assess_schema, SchemaStatus, MIGRATIONS};
use journey::{output_with_input, starts, Sandbox};
use runtime::{child_watchdog, Fixture};

type History = Vec<(String, i64, String)>;
/// The recorded job facts that exist in every schema version stepped to.
type Jobs = Vec<(String, String, Option<i64>, String, String)>;

fn history(db: &rusqlite::Connection) -> History {
    db.prepare("SELECT id,applied_at,checksum FROM schema_migrations ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn jobs(db: &rusqlite::Connection) -> Jobs {
    db.prepare(
        "SELECT l.job_id,j.job_class,j.exit_code,t.state,p.state FROM job_lifecycle l \
         JOIN jobs j ON j.id=l.job_id JOIN traffic_entries t ON t.job_id=l.job_id \
         JOIN permits p ON p.job_id=l.job_id ORDER BY t.sequence",
    )
    .unwrap()
    .query_map([], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

/// Stop the daemon and check its exit report. A daemon writes the report
/// once and never over an existing one, so it is set aside for the next start.
fn stop(f: &Fixture, daemon: &mut runtime::OwnedChild, run: usize) {
    daemon.stop(libc::SIGTERM);
    f.report();
    std::fs::rename(
        f.root.join("runtime.json"),
        f.root.join(format!("runtime-{run}.json")),
    )
    .unwrap();
}

/// `arcctl doctor` of the built CLI against the running daemon.
fn doctor(f: &Fixture, s: &Sandbox) -> String {
    let output = child_watchdog::output(s.arcctl(f, &s.home).arg("doctor"), "doctor").unwrap();
    assert!(
        output.status.success() && output.stderr.is_empty(),
        "{output:?}"
    );
    String::from_utf8(output.stdout).unwrap()
}

/// A database a real daemon wrote, put into the shape of an earlier schema
/// version, is migrated by the current daemon binary when it starts: the
/// daemon serves the current version, the earlier history rows are untouched
/// and the recorded job is the same row.
#[test]
fn seam_stepped_back_database_migrates_at_daemon_start() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    s.stub("cargo");
    let project = s.project("project");
    let marker = f.root.join("cargo-starts");
    let current = MIGRATIONS.last().unwrap().version;
    let compatible = format!(
        "ok. IPC owner-only. Tray not connected. Vendors not detected. Command classification \
         not checked. Sweeper off. Schema compatible. Version {current}. Migration history verified.\n"
    );

    // The real daemon creates the database and records one finished build.
    let mut daemon = f.spawn_isolated("");
    assert_eq!(doctor(&f, &s), compatible);
    let mut build = s.arcctl(&f, &project);
    build
        .args(["exec", "--", "cargo", "build"])
        .env("ARC_FIXTURE_MODE", "success")
        .env("ARC_FIXTURE_MARKER", &marker);
    let output = output_with_input(&mut build, b"", "cargo build");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(starts(&marker), 1);
    stop(&f, &mut daemon, 0);
    let db = journey::database(&f);
    let (recorded, written) = (jobs(&db), history(&db));
    drop(db);
    assert_eq!(recorded.len(), 1, "{recorded:?}");
    assert_eq!(
        (
            recorded[0].1.as_str(),
            recorded[0].2,
            recorded[0].3.as_str()
        ),
        ("BUILD", Some(0), "COMPLETED")
    );
    assert_eq!(recorded[0].4, "RELEASED");
    assert_eq!(written.len(), current as usize);

    for (run, version) in [current - 1, schema_step_back::EARLIEST]
        .into_iter()
        .enumerate()
    {
        schema_step_back::step_back(&f.database, version);
        let db = journey::database(&f);
        assert_eq!(jobs(&db), recorded, "step back to {version} lost a row");
        assert_eq!(history(&db), written[..version as usize]);
        drop(db);

        // The current daemon binary starts on it, migrates and serves.
        let mut daemon = f.spawn_isolated("");
        assert_eq!(doctor(&f, &s), compatible, "from version {version}");
        stop(&f, &mut daemon, run + 1);
        let db = journey::database(&f);
        let schema = assess_schema(&db).unwrap();
        assert_eq!(
            (
                schema.status,
                schema.applied_version,
                schema.history_verified
            ),
            (SchemaStatus::Compatible, Some(current), true)
        );
        let migrated = history(&db);
        assert_eq!(migrated[..version as usize], written[..version as usize]);
        assert_eq!(
            migrated
                .iter()
                .map(|row| row.0.as_str())
                .collect::<Vec<_>>(),
            MIGRATIONS.iter().map(|m| m.id).collect::<Vec<_>>()
        );
        assert_eq!(
            jobs(&db),
            recorded,
            "migration from {version} changed a row"
        );
    }
    assert_eq!(starts(&marker), 1, "a restart replayed the command");
}
