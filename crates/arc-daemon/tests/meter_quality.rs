//! Meter quality flags, allocated footprint, session usage and rollup
//! counts through built `arcctl exec`, the installed hook with recorded
//! Claude Code and Cursor payloads, a real daemon and offline stubs (lane B:
//! AT-MTR-02, MTR-015 item 3, G-01 items 3 and 4, R-005 item 3; cycle 35
//! follow-ups F2, F3, F4). Every wait is on observed state; the 60 s
//! deadlines are watchdogs only.
#[cfg(target_os = "macos")]
#[path = "meter_quality/cpu_gap.rs"]
mod cpu_gap;
#[allow(dead_code)] // Only the installed hook and its returned command are used here.
#[path = "hook_event_identity/hook.rs"]
mod hook;
#[path = "meter_quality/integration.rs"]
mod integration;
#[path = "support/journey.rs"]
mod journey;
#[path = "meter_quality/legacy.rs"]
mod legacy;
#[path = "meter_quality/overlap.rs"]
mod overlap;
#[path = "meter_quality/privacy.rs"]
mod privacy;
#[path = "meter_quality/quality.rs"]
mod quality;
#[allow(dead_code)] // Each scenario uses the helpers it needs.
#[path = "meter_quality/support.rs"]
mod quality_support;
#[allow(dead_code)] // Only the F3 CPU, its settling and the comparison are used here.
#[path = "usage_summary/recompute.rs"]
mod recompute;
#[allow(dead_code)] // Only the raw-id scans are used here.
#[path = "hook_event_identity/record.rs"]
mod record;
#[path = "meter_quality/rollup.rs"]
mod rollup;
#[allow(dead_code)] // The shared fixture also serves unrelated workloads.
#[path = "support/runtime.rs"]
mod runtime;
#[path = "meter_quality/sessions.rs"]
mod sessions;
mod support;
#[allow(dead_code)] // Reuse the existing unwaited-grandchild fixture for F3.
#[path = "job_process_meter/tree.rs"]
mod tree_fixture;

use journey::{output_with_input, Sandbox};
use runtime::{child_watchdog, Fixture};
use serde_json::Value;

/// A direct `arcctl exec` job (no agent hook) is recorded once and read back
/// through built `arcctl meter summary --by vendor`, grouped under `none`
/// in its registration hour, whichever summary version the daemon serves.
#[test]
fn seam_direct_exec_job_is_summarized_under_vendor_none() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    legacy::assert_legacy_bytes(&f, &s, &project);
    s.stub("cargo");
    let mut daemon = f.spawn_isolated("");

    let mut exec = s.arcctl(&f, &project);
    exec.args(["exec", "--", "cargo", "build"])
        .env("ARC_FIXTURE_MODE", "echo");
    let output = output_with_input(&mut exec, b"input\n", "arcctl exec");
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    let registered: i64 = journey::database(&f)
        .query_row("SELECT registered_at_ms FROM job_lifecycle", [], |r| {
            r.get(0)
        })
        .unwrap();
    let from = registered as u64 - registered as u64 % 3_600_000;
    let output = child_watchdog::output(
        s.arcctl(&f, &project).args([
            "meter",
            "summary",
            "--by",
            "vendor",
            "--from",
            &from.to_string(),
            "--to",
            &(from + 3_600_000).to_string(),
            "--bucket",
            "hour",
            "--json",
        ]),
        "arcctl meter summary",
    )
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    let summary: Value = serde_json::from_slice(&output.stdout).unwrap();
    let groups = summary["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 1, "{summary}");
    assert_eq!(groups[0]["vendor"], "none", "{summary}");
    assert_eq!(groups[0]["bucket_start_ms"], from, "{summary}");
    assert_eq!(groups[0]["local"]["job.count"]["total"], 1, "{summary}");
    daemon.stop(libc::SIGTERM);
    f.report();
}

/// AT-MTR-02, MTR-015 item 3, G-01 item 3, R-005 item 3 and cycle 35 F2,
/// F3 and F4 through built `arcctl exec`, the installed hooks with recorded
/// Claude Code and Cursor payloads and a real daemon:
///
/// - a build writes 32 MiB with fsync, deletes it and leaves 1 MiB in a
///   nonempty directory: Disk Written (cumulative, sampled) is at least
///   32 MiB, Current Footprint is the allocated bytes that remain, and the
///   footprint delta is at least 1 MiB and below the written bytes, each
///   equal to the test's own native observation and each its own field;
/// - its tree-sampled CPU is the summary CPU, labelled partial because its
///   waited usage exceeds what the samples saw (F4), while a job no sample
///   saw reports its waited CPU as a partial lower bound (F3);
/// - sessions group separately per digest and vendor, a missing session is
///   `none`, the totals equal SQLite, and no raw session or tool call id is
///   in the CLI's job diagnostics, the original daemon's own stdout, stderr
///   and runtime report (scanned before any restart recreates them), or
///   SQLite;
/// - every reported value is flagged, an unmeasured one unavailable;
/// - two nonempty rollup passes over the daemon's own samples count each
///   job once per hour and day, and repeating a pass changes nothing (F2).
#[test]
fn meter_quality_footprint_sessions_and_rollups_use_production_paths() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    s.stub("cargo");
    quality_support::compile(&f, &s);
    tree_fixture::compile(&f, &s);
    let gate = f.root.join("footprint-gate");
    std::fs::create_dir(&gate).unwrap();
    let mut command = f.isolated_command("");
    command
        .env("ARC_FOOTPRINT_FIXTURE_GATE", &gate)
        .env("ARC_METER_ACTIVE_1S", "1");
    let mut daemon = runtime::OwnedChild::new(command.spawn().unwrap());
    f.wait_for(&mut daemon, "arc-daemon ready.");
    let db = journey::database(&f);
    db.busy_timeout(std::time::Duration::from_secs(60)).unwrap();

    let footprint = quality::footprint(&f, &s, &db);
    let traffic_footprint = integration::traffic_footprint(&f, &s, &db);
    let unreserved_footprint = integration::unreserved_footprint(&f, &s, &db);
    let mut tree_job = quality::unwaited_tree(&f, &s, &db, "tree-cpu");
    let waited_only = quality::waited_only(&f, &s, &db);
    if !quality::same_day(&db, &tree_job, &waited_only) {
        // If the day changed between the jobs, obtain a new measured job in
        // the waited job's day instead of skipping the partial-group check.
        tree_job = quality::unwaited_tree(&f, &s, &db, "tree-cpu-next-day");
    }
    let sessions_project = s.project("sessions");
    let marker = f.root.join("cargo-starts");
    let events = sessions::deliver(&f, &s, &sessions_project, &marker);

    recompute::settle(&db);
    let range = quality::range(&db);
    quality::assert_footprint(&f, &s, &db, &footprint);
    quality::assert_footprint(&f, &s, &db, &traffic_footprint);
    quality::assert_footprint(&f, &s, &db, &unreserved_footprint);
    integration::assert_tree_quality(&f, &s, &footprint, &waited_only);
    integration::assert_missed_sample_is_partial(&f, &s, &db);
    #[cfg(target_os = "macos")]
    cpu_gap::missed_unwaited_child_stays_partial(&f, &s, &db);
    quality::assert_cpu_quality(&f, &s, &db, range, &footprint, &tree_job, &waited_only);
    sessions::assert_summary(&f, &s, &sessions_project, &db, range);
    quality::assert_completion_replay_keeps_original_footprint(&f, &s, &db);
    integration::assert_traffic_replay_keeps_original_footprint(&f, &s, &db);
    overlap::traffic_grant_completion_and_cancelled_walk(&f, &s, &db, &gate);
    overlap::recovery_cannot_replace_captured_end(&f, &s, &db, &gate);
    let session_jobs = privacy::session_jobs(&db);
    privacy::assert_diagnostics_hide_raw_ids(&f, &s, &sessions_project, &session_jobs, &events);
    rollup::assert_rollups_count_each_job_once(&f, &s, &db, &mut daemon, || {
        privacy::assert_original_daemon_kept_no_raw_ids(&f, &db, &session_jobs, &events)
    });

    // The whole fixture, including the preserved original daemon output.
    for (_, kept) in privacy::ORIGINAL_LOGS {
        assert!(f.root.join(kept).is_file(), "{kept} not preserved");
    }
    record::assert_no_raw_ids_of_any(&f, &events);
    drop(db);
    record::assert_no_raw_ids_of_any(&f, &events);
}
