//! Shared steps of the lane C tests: a real daemon with explicit startup
//! configuration, built `arcctl` commands with offline stubs, raw policy
//! requests and daemon-state reads. Every process gets a cleared environment.
use crate::{
    journey::{self, output_with_input, Sandbox},
    runtime::{child_watchdog, Fixture, OwnedChild},
};
use arc_ipc::{governor::*, Request};
use serde_json::Value;
use std::{
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub const START_RATE: &str = "ARC_TRAFFIC_START_RATE_PER_MINUTE";
pub const START_BURST: &str = "ARC_TRAFFIC_START_BURST";
pub const CPU_BUDGET: &str = "ARC_TRAFFIC_CPU_BUDGET_MILLIS";

/// Start the production daemon with only fixture configuration plus `env`.
pub fn daemon(f: &Fixture, env: &[(&str, &str)]) -> OwnedChild {
    let mut command = f.isolated_command("");
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = OwnedChild::new(command.spawn().unwrap());
    f.wait_for(&mut child, "arc-daemon ready.");
    child
}

/// Stop, check the runtime report and database, and make room for a restart.
pub fn stop(f: &Fixture, mut daemon: OwnedChild) {
    daemon.stop(libc::SIGTERM);
    f.report();
    std::fs::remove_file(f.root.join("runtime.json")).unwrap();
}

/// `arcctl exec -- cargo <verb>` in `project`; the offline stub records each
/// of its starts in `marker`.
pub fn cargo(f: &Fixture, s: &Sandbox, project: &Path, verb: &str, marker: &Path) -> Command {
    let mut command = s.arcctl(f, project);
    command
        .args(["exec", "--", "cargo", verb])
        .env("ARC_FIXTURE_MODE", "success")
        .env("ARC_FIXTURE_MARKER", marker)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    command
}

/// Run one `arcctl exec -- cargo <verb>` to its successful end.
pub fn run(f: &Fixture, s: &Sandbox, project: &Path, verb: &str, marker: &Path) {
    let output =
        child_watchdog::output(&mut cargo(f, s, project, verb, marker), "arcctl exec").unwrap();
    assert!(output.status.success(), "{output:?}");
}

/// `arcctl job diagnostics <job> [--json]` through the built CLI.
pub fn diagnostics(f: &Fixture, s: &Sandbox, job: &str, json: bool) -> String {
    let mut command = s.arcctl(f, &s.home);
    command.args(["job", "diagnostics", job]);
    if json {
        command.arg("--json");
    }
    let output = output_with_input(&mut command, b"", "job diagnostics");
    assert!(
        output.status.success() && output.stderr.is_empty(),
        "{output:?}"
    );
    String::from_utf8(output.stdout).unwrap()
}

pub fn diagnostics_json(f: &Fixture, s: &Sandbox, job: &str) -> Value {
    serde_json::from_str(&diagnostics(f, s, job, true)).unwrap()
}

/// One raw request on an authenticated connection, as any client sends it.
pub fn rpc(f: &Fixture, operation: &str, payload: Value) -> Value {
    let mut request = Request::new("traffic-policy", operation);
    request.payload = payload;
    let mut session = arc_ipc::client::Session::connect_until(
        &f.socket,
        Instant::now() + Duration::from_secs(10),
        &|| false,
        None,
    )
    .unwrap();
    session.exchange(&request, &|| false).unwrap()
}

/// Set `key` to `enabled` through `operation` at `base` with a unique
/// `nonce`, and return the committed switch.
pub fn set(
    f: &Fixture,
    operation: &str,
    key: GovernorSwitchKey,
    enabled: bool,
    base: u64,
    nonce: u32,
) -> GovernorSwitchState {
    let request = SetGovernorSwitchRequest {
        key,
        enabled,
        expected_revision: base,
        mutation_id: format!("{base}-{nonce:032x}"),
    };
    let result =
        decode_mutation_result(&rpc(f, operation, serde_json::json!(request)), &request).unwrap();
    let GovernorMutationResult::Committed {
        switch,
        changed: true,
        ..
    } = result
    else {
        panic!("{result:?}");
    };
    switch
}

/// The job of the wrapper process `observer`.
pub fn job_of(db: &rusqlite::Connection, observer: u32) -> String {
    db.query_row(
        "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1",
        [observer],
        |row| row.get(0),
    )
    .unwrap()
}

/// Every recorded decision of `job`, oldest first, as SQLite holds it.
pub fn decisions(db: &rusqlite::Connection, job: &str) -> Vec<(String, String, i64, String)> {
    db.prepare(
        "SELECT decision,reason_code,recorded_at_ms,transition_key FROM policy_decisions
         WHERE job_id=?1 ORDER BY rowid",
    )
    .unwrap()
    .query_map([job], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

/// The open database, with a long busy timeout for reads beside the daemon.
pub fn database(f: &Fixture) -> rusqlite::Connection {
    let db = journey::database(f);
    db.busy_timeout(Duration::from_secs(60)).unwrap();
    db
}

/// The conservative default of every class (POL-001), as the history read
/// reports it: window 1, the fixed per-class estimates and 120 starts per
/// minute with a burst of 120.
pub fn assert_default_class(class: &Value, name: &str) {
    let gib = 1024_u64 * 1024 * 1024;
    let (cpu, memory) = if name == "BUILD" {
        (2_000, 2 * gib)
    } else {
        (1_000, gib)
    };
    assert_eq!(
        *class,
        serde_json::json!({"class": name, "window": 1, "cpu_millis_estimate": cpu,
            "memory_bytes_estimate": memory, "start_tokens_per_minute": 120, "start_burst": 120}),
    );
}
