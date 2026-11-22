//! Job process-tree metering through built `arcctl exec`, a real daemon and
//! offline fixtures (lane A: MTR-002, MTR-003, MET-002, MET-003, MET-004,
//! MET-005, DET-012, MET-009, MET-010, MTR-009). Every wait is on observed
//! state; the 60 s deadlines are diagnostic watchdogs.
#[path = "support/journey.rs"]
mod journey;
#[allow(dead_code)] // The shared fixture also serves unrelated workloads.
#[path = "support/runtime.rs"]
mod runtime;
mod support;
#[path = "job_process_meter/tree.rs"]
mod tree;

use arc_core::job_meter::{
    AttributionBasis, Completeness, JobMeterReport, JobMeterSummary, MissingCoverageReason,
    SummaryAccounting,
};
use arc_ipc::meter::job_tree::{ProcessTreeReport, TreeBasis, SUMMARY_KEY};
use journey::{output_with_input, Sandbox};
use runtime::{child_watchdog, Fixture};
use std::{collections::BTreeMap, path::Path, time::Duration};

/// `arcctl meter summary --job --json`, split into the v1 summary and the
/// one added `process_tree` value. The v1 part still parses strictly.
fn summary(
    s: &Sandbox,
    f: &Fixture,
    cwd: &Path,
    job: &str,
) -> (JobMeterSummary, serde_json::Value) {
    let output = child_watchdog::output(
        s.arcctl(f, cwd)
            .args(["meter", "summary", "--job", job, "--json"]),
        "meter summary",
    )
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    let mut value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let tree = value
        .as_object_mut()
        .unwrap()
        .remove(SUMMARY_KEY)
        .expect("the daemon advertises meter.job-tree.v1");
    (serde_json::from_value(value).unwrap(), tree)
}

fn recorded(summary: &JobMeterSummary) -> Option<&JobMeterReport> {
    match &summary.accounting {
        SummaryAccounting::Recorded { report } => Some(report),
        _ => None,
    }
}

/// A direct `arcctl exec` job's v1 summary, read through built `arcctl meter
/// summary --job --json`, is today's waited-descendant report of that job.
#[test]
fn seam_arcctl_exec_job_meter_summary_reads_waited_report() {
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

    let mut read = None;
    journey::eventually("recorded job Meter report", || {
        let (v1, tree) = summary(&s, &f, &project, &job);
        let done = recorded(&v1).is_some();
        read = Some((v1, tree));
        done
    });
    let (summary, _) = read.unwrap();
    assert_eq!(summary.job_id, job);
    let report = recorded(&summary).unwrap();
    assert_eq!(report.job_id, job);
    assert_eq!(
        report.attribution_basis,
        AttributionBasis::NativeOwnedChildWait4
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

/// The managed root starts, in its process group, a CPU-burning,
/// memory-holding, disk-writing grandchild and never waits for it. The
/// daemon's own Meter cadence samples the job's tree from the recorded child
/// identity; the totals read back through built `arcctl meter summary --job
/// --json`. A process outside the tree and a running job whose recorded
/// child identity names another process (a recycled PID) are never
/// attributed.
#[test]
fn mtr_002_unwaited_grandchild_cpu_memory_and_io_are_attributed_to_the_job() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    tree::compile(&f, &s);
    let mut daemon = f.spawn_isolated("");
    let mut db = journey::database(&f);
    db.busy_timeout(Duration::from_secs(60)).unwrap();
    let exec = |directory: &Path, mode: &str| {
        let mut command = s.arcctl(&f, &project);
        command
            .args(["exec", "--", tree::PROGRAM, mode])
            .arg(directory)
            .stdin(std::process::Stdio::null());
        child_watchdog::Owned::spawn(&mut command).unwrap()
    };

    // Recycled-identity control: a RUNNING job whose recorded child identity
    // now names another process at the same PID.
    let sleeper_dir = tree::directory(&f, "sleeper");
    let mut sleeper = exec(&sleeper_dir, "sleeper");
    let sleeper_pid = tree::published(&sleeper_dir, "sleeper.pid");
    let sleeper_job = tree::running_job(&db, sleeper_pid);
    let (sleeper_token, sleeper_rows) = tree::recycle(&mut db, &sleeper_job);

    // Outside control: a CPU-burning process in no job's tree.
    let outside_dir = tree::directory(&f, "outside");
    let mut outside = child_watchdog::Owned::spawn(
        s.command(&f, s.bin.join(tree::PROGRAM), &project)
            .arg("outside")
            .arg(&outside_dir),
    )
    .unwrap();
    let outside_pid = tree::published(&outside_dir, "outside.pid");

    let job_dir = tree::directory(&f, "job");
    let mut root_exec = exec(&job_dir, "root");
    let root_pid = tree::published(&job_dir, "root.pid");
    let grandchild_pid = tree::published(&job_dir, "grandchild.pid");
    let _grandchild = tree::Reaped::capture(grandchild_pid);
    let held = tree::published(&job_dir, "held");
    let written = tree::published(&job_dir, "written");
    let read_back = tree::published(&job_dir, "read");
    let job = tree::running_job(&db, root_pid);
    let is_grandchild = |row: &tree::Row| u64::from(row.identity.pid) == grandchild_pid;

    // The grandchild spans at least two samples, observed from daemon state,
    // and a sample reads its held resident bytes and the bytes it read back.
    // These are exact floors: across ten fixture runs, five of them beside
    // four CPU-bound processes, the grandchild's resident size never read
    // below HELD and its disk reads never below the bytes read back.
    journey::eventually("two samples of the unwaited grandchild", || {
        let rows = tree::rows(&db, &job);
        let grandchild: Vec<_> = rows.iter().filter(|r| is_grandchild(r)).collect();
        grandchild.len() >= 2
            && grandchild
                .iter()
                .any(|r| r.rss_bytes >= held && r.disk_read_bytes >= read_back)
    });
    tree::release(&job_dir);
    tree::release(&outside_dir);
    assert!(root_exec.wait("managed root").unwrap().success());
    assert!(outside.wait("outside control").unwrap().success());
    tree::restore(&db, &sleeper_job, &sleeper_token);
    tree::release(&sleeper_dir);
    assert!(sleeper.wait("recycled control").unwrap().success());

    // The terminal report, completed with the root's waited usage.
    let mut read = None;
    journey::eventually("terminal process-tree report", || {
        let (v1, value) = summary(&s, &f, &project, &job);
        let report: Option<ProcessTreeReport> = serde_json::from_value(value).unwrap();
        let done = recorded(&v1).is_some()
            && report.as_ref().is_some_and(|r| {
                r.terminal && r.waited_cpu_core_seconds.basis == TreeBasis::WaitedRusage
            });
        read = Some((v1, report));
        done
    });
    let (v1, report) = read.unwrap();
    let report = report.unwrap();
    let waited = recorded(&v1).unwrap().cpu_core_seconds.total.unwrap();
    let rows = tree::rows(&db, &job);

    // The grandchild is attributed to the job, Estimated (C); the root is A.
    let grandchild: Vec<_> = rows.iter().filter(|r| is_grandchild(r)).collect();
    assert!(grandchild.len() >= 2, "{rows:?}");
    assert!(grandchild.iter().all(|r| r.confidence == "C"));
    assert!(rows
        .iter()
        .filter(|r| u64::from(r.identity.pid) == root_pid)
        .all(|r| r.confidence == "A"));
    let distinct: std::collections::BTreeSet<_> = grandchild
        .iter()
        .map(|r| (&r.identity.start_token, &r.identity.boot_id))
        .collect();
    assert_eq!(distinct.len(), 1, "one revalidated identity");

    // Job CPU includes the unwaited grandchild and exceeds the waited rusage.
    assert_eq!(report.waited_cpu_core_seconds.value, Some(waited));
    let cpu = report.cpu_core_seconds.value.unwrap();
    let unwaited = report.unwaited_cpu_core_seconds.value.unwrap();
    assert_eq!(report.cpu_core_seconds.basis, TreeBasis::TreeSamples);
    assert!(cpu > waited, "tree {cpu} vs waited {waited}");
    assert!((cpu - (waited + unwaited)).abs() < 1e-9);
    let last_grandchild = grandchild.last().unwrap();
    assert!(unwaited + 1e-9 >= last_grandchild.cpu_time_ns as f64 / 1e9);

    // Peak memory: the highest summed RSS of one sample of the job.
    let mut per_sample = BTreeMap::<u64, u64>::new();
    for row in &rows {
        *per_sample.entry(row.timestamp_ns).or_default() += row.rss_bytes;
    }
    let peak = report.memory_peak_bytes.value.unwrap();
    assert_eq!(Some(&peak), per_sample.values().max());
    let grandchild_rss = grandchild.iter().map(|r| r.rss_bytes).max().unwrap();
    assert!(grandchild_rss >= held, "{grandchild_rss} < held {held}");
    assert!(peak >= grandchild_rss && peak >= held, "peak {peak}");

    // Read and cumulative written bytes: each sampled process's last reading.
    let mut last = BTreeMap::new();
    for row in &rows {
        last.insert((row.identity.pid, row.identity.start_token.clone()), row);
    }
    let sum = |value: fn(&tree::Row) -> u64| last.values().map(|r| value(r)).sum::<u64>();
    assert_eq!(
        report.disk_write_bytes.value,
        Some(sum(|r| r.disk_write_bytes))
    );
    assert_eq!(
        report.disk_read_bytes.value,
        Some(sum(|r| r.disk_read_bytes))
    );
    assert!(report.disk_write_bytes.value.unwrap() >= written);
    assert!(last_grandchild.disk_write_bytes >= written);
    assert!(report.disk_read_bytes.value.unwrap() >= read_back);
    assert!(last_grandchild.disk_read_bytes >= read_back);

    // Per-process utilization in core-percent: 100 is one core, from
    // consecutive samples of one identity.
    let cores = std::thread::available_parallelism().unwrap().get() as f64;
    for pair in grandchild.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let expected = (b.cpu_time_ns - a.cpu_time_ns) as f64
            / (b.timestamp_ns - a.timestamp_ns) as f64
            * 100.0;
        let percent = b.cpu_core_percent.unwrap();
        assert!((percent - expected).abs() <= expected.abs() * 1e-9 + 1e-9);
        assert!(percent > 0.0 && percent <= 100.0 * cores, "{percent}");
    }
    assert_eq!(grandchild[0].cpu_core_percent, None, "first sample");

    // A descendant may have exited between samples: partial, never zero.
    assert_eq!(report.completeness, Completeness::Partial);
    assert!(report
        .missing_coverage
        .contains(&MissingCoverageReason::UnobservedDescendants));
    assert!(report.samples >= 2 && report.processes >= 2);

    // The arcctl read is the stored terminal report.
    let stored: String = db
        .query_row(
            "SELECT process_tree_json FROM job_meter_summaries WHERE job_id=?1",
            [&job],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<ProcessTreeReport>(&stored).unwrap(),
        report
    );

    // Neither control is ever attributed.
    assert!(!tree::sampled_pids(&db).contains(&(outside_pid as u32)));
    let after: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM resource_samples WHERE job_id=?1",
            [&sleeper_job],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(after, sleeper_rows, "recycled identity sampled");

    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}
