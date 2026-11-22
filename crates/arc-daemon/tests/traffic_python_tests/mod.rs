//! Shared handoff for daemon and registered desktop acceptance. No GUI needed.
//! Read must route traffic_job_diagnostics and meter_summary to production APIs.
mod contention;
mod execution;
mod faults;
mod processes;
mod projects;
#[path = "runtime.rs"]
mod python;
mod recovery;
#[path = "../traffic_bun_scripts/mod.rs"]
mod shared;

use crate::{cleanup, native};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use projects::{Kind, Tools};
use serde_json::{json, Value};
pub use shared::Read;
use shared::{binary, eventually, exchange, executable, log, sha, write, Case as NativeCase, Run};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
type Case = NativeCase<Tools>;

#[derive(Debug, Clone, Copy)]
pub enum Peer {
    Cargo,
    Bun,
}

pub fn direct_read(socket: &Path, command: &str, args: Value) -> Value {
    let operation = match command {
        "traffic_job_diagnostics" => {
            assert_eq!(
                exchange(
                    socket,
                    "traffic.python-tests.diagnostics.capabilities",
                    Value::Null
                ),
                json!({"capability":"traffic.python-tests.diagnostics.v1"})
            );
            "traffic.python-tests.diagnostics.job"
        }
        "meter_summary" => "meter.summary",
        _ => panic!("unsupported Python fixture read: {command}"),
    };
    exchange(socket, operation, args)
}

fn case(name: &str, read: Read, injected: bool) -> Case {
    let tools = Tools::installed();
    let receipt = tools.receipt();
    let observers = tools.observers();
    NativeCase::with_tools(
        &format!("python-{name}"),
        read,
        injected,
        tools,
        receipt,
        observers,
    )
}

/// Stable IDs, SQLite/CLI/Meter assertions and callback all live in this suite.
/// The desktop target calls these exact scenarios using its registered handler.
pub fn semantics(read: Read) {
    execution::semantics(read);
}
pub fn lifecycle(read: Read) {
    contention::global_budget(read);
    recovery::governor_pressure(read);
    recovery::cancel_restart(read);
    processes::signals(read);
    processes::nested(read);
    faults::acknowledgments(read);
    faults::compatibility_and_outage(read);
}
pub fn one_test_slot(read: Read, peer: Peer, python_first: bool) {
    contention::pair(read, peer, python_first);
}

fn start(case: &Case, path: &Path, argv: &[&str]) -> Run {
    start_with(case, path, argv, |_| {})
}
fn start_with(
    case: &Case,
    path: &Path,
    argv: &[&str],
    configure: impl FnOnce(&mut Command),
) -> Run {
    Run::start_with(case, path, argv, |cmd| {
        projects::configure(case, path, cmd);
        configure(cmd);
    })
}
fn entered(case: &Case, run: &mut Run, kind: Kind) -> Value {
    eventually(|| {
        assert!(
            run.wrapper.child.try_wait().unwrap().is_none(),
            "real test exited before explicit release: {} {}",
            log(&run.project.join("stdout")),
            log(&run.project.join("stderr"))
        );
        log(&run.project.join("bodies")).lines().count() == 1
    });
    let id = case.job(run.wrapper.child.id());
    eventually(|| case.row(&id)["state"] == "RUNNING");
    let row = case.row(&id);
    let identity = ProcessIdentity::capture(row["child_pid"].as_u64().unwrap() as u32).unwrap();
    assert_eq!(probe_live(&identity), IdentityStatus::VerifiedMatch);
    run.wrapper.own_process(identity.clone());
    assert_eq!(log(&run.project.join("launches")).lines().count(), 1);
    let exe = executable(identity.pid);
    assert_eq!(exe, kind.executable(case).canonicalize().unwrap());
    let facts = case.reconcile(&id, kind.class(), kind.reason());
    json!({"jobId":id,"child":identity,"executable":exe,"facts":facts,"starts":1,"bodyEntries":1})
}
fn queued(case: &Case, run: &mut Run, kind: Kind, wait: &str) -> Value {
    let id = case.job(run.wrapper.child.id());
    eventually(|| {
        case.row(&id)["state"] == "QUEUED"
            && log(&run.project.join("stderr")).contains("QUEUED\nWaiting to start.")
    });
    assert_eq!(log(&run.project.join("launches")).lines().count(), 0);
    assert_eq!(log(&run.project.join("bodies")).lines().count(), 0);
    assert!(
        cleanup::children(run.wrapper.child.id())
            .unwrap()
            .is_empty(),
        "durably queued wrapper has no original child"
    );
    let mut snapshot = case.reconcile(&id, kind.class(), kind.reason());
    let meter = meter(case, &id);
    assert_eq!(
        meter["accounting"],
        json!({"state":"unknown","reason":"not_terminated"})
    );
    snapshot["meter"] = meter;
    assert!(snapshot["sqlite"]["child_pid"].is_null());
    assert_eq!(snapshot["diagnostic"]["admission"]["permit_active"], false);
    assert_eq!(snapshot["diagnostic"]["admission"]["wait_reason"], wait);
    snapshot
}
fn meter(case: &Case, id: &str) -> Value {
    let cli = case.cli(&["meter", "summary", "--job", id, "--json"]);
    assert!(cli.status.success(), "{cli:?}");
    let mut meter: Value = serde_json::from_slice(&cli.stdout).unwrap();
    // The CLI adds the job's process tree beside the v1 summary (MTR-002).
    let tree = meter.as_object_mut().unwrap().remove("process_tree");
    assert!(tree.is_some_and(|t| t.is_object() || t.is_null()));
    assert_eq!(
        meter,
        (case.read)(&case.fixture.socket, "meter_summary", json!({"job_id":id}))
    );
    meter
}
fn terminal(case: &Case, run: &Run, kind: Kind, code: i32) -> Value {
    let result = case.terminal(run, kind.class(), kind.reason(), code);
    assert_eq!(log(&run.project.join("bodies")).lines().count(), 1);
    assert_eq!(log(&run.project.join("launches")).lines().count(), 1);
    result
}

/// Reconstruct every complete persisted permit interval, rather than relying
/// only on sampled ACTIVE counts. Release precedes any equal-time new grant.
fn permit_history(case: &Case, expected: usize, max_tests: usize) -> Value {
    let rows = case.rows("SELECT p.job_id,j.job_class,p.state,p.generation,p.released_at,t.granted_at_ms,p.cpu_millis_est,p.memory_bytes_est FROM permits p JOIN jobs j ON j.id=p.job_id JOIN traffic_entries t ON t.job_id=p.job_id ORDER BY t.granted_at_ms,p.job_id");
    assert_eq!(rows.len(), expected);
    let mut events = Vec::new();
    for row in &rows {
        assert_eq!(row["state"], "RELEASED");
        let grant = row["granted_at_ms"].as_u64().unwrap();
        let release = row["released_at"].as_u64().unwrap();
        assert!(release >= grant);
        let test = row["job_class"] == "TEST";
        if grant < release {
            events.extend([(grant, 1_i32, test), (release, -1_i32, test)]);
        }
    }
    events.sort_by_key(|&(time, delta, _)| (time, delta));
    let (mut test, mut high) = (0_i32, 0);
    for &(_, delta, is_test) in &events {
        if is_test {
            test += delta;
            high = high.max(test);
            assert!((0..=max_tests as i32).contains(&test));
        }
    }
    assert_eq!(test, 0);
    assert_eq!(case.active()["permits"], 0);
    json!({"allIntervals":rows,"testPermitHighWater":high,"released":expected})
}
