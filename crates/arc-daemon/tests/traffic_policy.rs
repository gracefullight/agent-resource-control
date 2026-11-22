//! Traffic start-rate control, decision history, policy-change audit and the
//! rollup and retention of raw samples, through built `arcctl` and a real
//! daemon with offline stubs (lane C: DEC-013, POL-001, TRF-015, POL-004,
//! MTR-014, NFR-023). Correctness never comes from elapsed time.
#[path = "traffic_policy/audit.rs"]
mod audit;
#[path = "traffic_policy/fixture.rs"]
mod fixture;
#[path = "traffic_policy/history.rs"]
mod history;
#[path = "support/journey.rs"]
mod journey;
#[allow(dead_code)] // Only the admission barrier is used here.
#[path = "support/native_exec.rs"]
mod native_exec;
#[path = "traffic_policy/retention.rs"]
mod retention;
#[allow(dead_code)] // The shared fixture also serves unrelated workloads.
#[path = "support/runtime.rs"]
mod runtime;
#[path = "traffic_policy/start_rate.rs"]
mod start_rate;
mod support;

use journey::{output_with_input, Sandbox};
use runtime::{child_watchdog, Fixture};

/// `arcctl job diagnostics` of a completed direct `cargo build` job reads its
/// last recorded decision from the real daemon, as text and as JSON.
#[test]
fn seam_job_diagnostics_reads_last_recorded_decision() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    s.stub("cargo");
    let mut daemon = f.spawn_isolated("");

    let mut exec = s.arcctl(&f, &project);
    exec.args(["exec", "--", "cargo", "build"])
        .env("ARC_FIXTURE_MODE", "echo");
    let output = output_with_input(&mut exec, b"input\n", "arcctl exec");
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    let job: String = journey::database(&f)
        .query_row("SELECT id FROM jobs", [], |row| row.get(0))
        .unwrap();

    let text = child_watchdog::output(
        s.arcctl(&f, &project).args(["job", "diagnostics", &job]),
        "job diagnostics",
    )
    .unwrap();
    assert!(text.status.success(), "{text:?}");
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains(&format!("Job: {job}\n")), "{text}");
    assert!(text.contains("Classification: BUILD\n"), "{text}");
    assert!(text.contains("\nLast recorded decision: "), "{text}");

    let json = child_watchdog::output(
        s.arcctl(&f, &project)
            .args(["job", "diagnostics", &job, "--json"]),
        "job diagnostics --json",
    )
    .unwrap();
    assert!(json.status.success(), "{json:?}");
    let document: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(document["job_id"], job.as_str());
    assert!(
        document["admission"]["last_decision"].is_object(),
        "{document}"
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

/// DEC-013 item 4 and POL-001 item 2. A Build bucket of one token per 10 s,
/// set through the startup Traffic configuration, holds the next build QUEUED
/// with the start-rate reason while its window is free; its own poll is then
/// granted once, with one permit and one start, after the token time recorded
/// in daemon state. A default daemon reports the conservative defaults of
/// every class, including its per-class estimates.
#[test]
fn dec_013_start_rate_bucket_queues_then_grants_without_lost_or_duplicated_permit() {
    let _serial = support::serial();
    start_rate::bucket_queues_then_grants_without_lost_or_duplicated_permit();
}

/// TRF-015 item 3. `arcctl job diagnostics` reads every recorded decision
/// of a job in recorded order, with its queue position while queued and no
/// invented schedule, and the whole sequence after it ran.
#[test]
fn trf_015_job_diagnostics_reads_every_decision_in_order_and_schedule() {
    let _serial = support::serial();
    history::diagnostics_reads_every_decision_in_order_and_schedule();
}

/// POL-004 item 3. Switch, pause and resume, and a changed startup
/// configuration (windows, budgets, start-rate settings), each write one
/// audit row with before and after values. No learned value exists.
#[test]
fn pol_004_every_policy_change_writes_before_and_after_audit() {
    let _serial = support::serial();
    audit::every_policy_change_writes_before_and_after_audit();
}

/// MTR-014 items 1-2 and NFR-023 item 1. Raw samples older than 72 h seeded
/// before each daemon start are rolled up with exact sums and deleted by the
/// daemon's own schedule, a stored Meter record expires, and row counts stay
/// bounded across three seeded batches.
#[test]
fn mtr_014_raw_samples_roll_up_then_delete_and_rows_stay_bounded() {
    let _serial = support::serial();
    retention::raw_samples_roll_up_then_delete_and_rows_stay_bounded();
}
