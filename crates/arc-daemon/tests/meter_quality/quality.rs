//! Footprint, CPU quality and rollup scenarios of the `meter_quality`
//! target, each run through built `arcctl exec` against the real daemon.
use crate::{
    journey::{self, Sandbox},
    quality_support::{self as support, MIB, PROGRAM},
    recompute::{DAY_MS, HOUR_MS},
    runtime::{child_watchdog, Fixture},
    tree_fixture,
};
use rusqlite::Connection;
use serde_json::Value;
use std::{path::PathBuf, process::Stdio};

const BIG: u64 = 32 * MIB;
const LEFT: u64 = MIB;
const EXISTING: u64 = 2 * MIB;

pub struct FootprintJob {
    pub job: String,
    pub project: PathBuf,
    /// The test's own allocated-bytes observations before and after the job.
    pub before: u64,
    pub after: u64,
    pub written: u64,
}

/// Start from a nonempty allocated directory; the build writes 32 MiB with
/// fsync, deletes it and leaves 1 MiB. The job ends only after a sample of
/// daemon state shows its written bytes.
pub fn footprint(f: &Fixture, s: &Sandbox, db: &Connection) -> FootprintJob {
    let project = s.project("build");
    support::write_synced(&project.join("existing.bin"), EXISTING);
    let control = support::directory(f, "footprint-control");
    let before = support::allocated(&project);
    assert!(before >= EXISTING, "{before}");
    let mut command = s.arcctl(f, &project);
    command
        .args(["exec", "--", PROGRAM, "footprint"])
        .arg(&control)
        .stdin(Stdio::null());
    let mut exec = child_watchdog::Owned::spawn(&mut command).unwrap();
    let pid = support::published(&control, "footprint.pid");
    let written = support::published(&control, "written");
    assert_eq!(written, BIG + LEFT);
    assert_eq!(support::published(&control, "left"), LEFT);
    let job = support::running_job(db, pid);
    journey::eventually("a sample with the written bytes", || {
        support::samples(db, &job).1 >= written
    });
    support::release(&control);
    assert!(exec.wait("footprint build").unwrap().success());
    let after = support::allocated(&project);
    FootprintJob {
        job,
        project,
        before,
        after,
        written,
    }
}

pub struct WaitedOnlyJob {
    pub job: String,
    pub project: PathBuf,
}

pub struct TreeJob {
    job: String,
}

/// A real detached grandchild contributes sampled CPU which wait4 cannot
/// include in the root's waited usage. Reuse the existing native fixture.
pub fn unwaited_tree(f: &Fixture, s: &Sandbox, db: &Connection, name: &str) -> TreeJob {
    let project = s.project(name);
    let control = tree_fixture::directory(f, &format!("{name}-control"));
    let mut command = s.arcctl(f, &project);
    command
        .args(["exec", "--", tree_fixture::PROGRAM, "root"])
        .arg(&control)
        .stdin(Stdio::null());
    let mut exec = child_watchdog::Owned::spawn(&mut command).unwrap();
    let root = tree_fixture::published(&control, "root.pid");
    let grandchild = tree_fixture::published(&control, "grandchild.pid");
    let grandchild_owner = tree_fixture::Reaped::capture(grandchild);
    let job = tree_fixture::running_job(db, root);
    journey::eventually("two samples of the unwaited CPU burner", || {
        let rows = tree_fixture::rows(db, &job);
        rows.iter()
            .filter(|row| u64::from(row.identity.pid) == grandchild && row.cpu_time_ns > 0)
            .count()
            >= 2
    });
    tree_fixture::release(&control);
    assert!(exec.wait("unwaited tree job").unwrap().success());
    journey::eventually("unwaited fixture process exited", || {
        arc_core::process_identity::probe_live(&grandchild_owner.0)
            != arc_core::process_identity::IdentityStatus::VerifiedMatch
    });
    TreeJob { job }
}

pub fn same_day(db: &Connection, sampled: &TreeJob, unsampled: &WaitedOnlyJob) -> bool {
    let registered = |job: &str| -> i64 {
        db.query_row(
            "SELECT registered_at_ms FROM job_lifecycle WHERE job_id=?1",
            [job],
            |r| r.get(0),
        )
        .unwrap()
    };
    registered(&sampled.job) / DAY_MS as i64 == registered(&unsampled.job) / DAY_MS as i64
}

/// A job that exits before any sample: its tree was never sampled and its
/// waited report is recorded. A job that a sample did catch, or whose
/// best-effort report was lost, is replaced by another in a new project.
pub fn waited_only(f: &Fixture, s: &Sandbox, db: &Connection) -> WaitedOnlyJob {
    for attempt in 0..20 {
        let project = s.project(&format!("quick-{attempt}"));
        let mut command = s.arcctl(f, &project);
        command
            .args(["exec", "--", PROGRAM, "quick"])
            .stdin(Stdio::null());
        let output = child_watchdog::output(&mut command, "quick job").unwrap();
        assert!(output.status.success(), "{output:?}");
        let (job, unsampled): (String, bool) = db
            .query_row(
                "SELECT j.id, m.process_tree_json IS NULL AND m.report_json IS NOT NULL
                   AND m.relation='canonical_root'
                 FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id
                 LEFT JOIN job_meter_summaries m ON m.job_id=j.id
                 ORDER BY l.registered_at_ms DESC, l.rowid DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get::<_, Option<bool>>(1)?.unwrap_or(false))),
            )
            .unwrap();
        if unsampled {
            return WaitedOnlyJob { job, project };
        }
    }
    panic!("every quick job was sampled or lost its report");
}

/// Whole hours covering every recorded job.
pub fn range(db: &Connection) -> (u64, u64) {
    let (first, last): (i64, i64) = db
        .query_row(
            "SELECT min(registered_at_ms),max(registered_at_ms) FROM job_lifecycle",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let (first, last) = (first as u64, last as u64);
    (first - first % HOUR_MS, last - last % HOUR_MS + HOUR_MS)
}

/// AT-MTR-02: Disk Written, Current Footprint and its delta are separate
/// fields, each against its own observation.
pub fn assert_footprint(f: &Fixture, s: &Sandbox, db: &Connection, job: &FootprintJob) {
    let read = |footprint: bool| {
        let mut args = vec!["meter", "summary", "--job", &job.job];
        if footprint {
            args.push("--footprint");
        }
        args.push("--json");
        support::arcctl_json(f, s, &job.project, &args)
    };
    // The default job report keeps its bytes: no footprint key.
    assert!(read(false).get("footprint").is_none());
    let report = read(true);
    let written = &report["process_tree"]["disk_write_bytes"];
    assert_eq!(written["basis"], "tree_samples", "{report}");
    let disk_written = written["value"].as_u64().unwrap();
    assert!(
        disk_written >= BIG && disk_written >= job.written,
        "{report}"
    );
    let footprint = &report["footprint"];
    for (key, expected) in [
        ("start_bytes", job.before as i64),
        ("end_bytes", job.after as i64),
        ("delta_bytes", job.after as i64 - job.before as i64),
    ] {
        let value = &footprint[key];
        assert_eq!(value["basis"], "allocated", "{key}: {footprint}");
        assert_eq!(value["availability"], "complete", "{key}: {footprint}");
        assert_eq!(
            value["value"].as_i64(),
            Some(expected),
            "{key}: {footprint}"
        );
    }
    let (end, delta) = (job.after, job.after - job.before);
    // What remains, beside the pre-existing bytes, is the 1 MiB left behind:
    // well below the cumulative 33 MiB written, which the delta never is.
    assert!(job.before >= EXISTING && end > job.before);
    assert!(
        delta >= LEFT && delta < disk_written,
        "{delta} {disk_written}"
    );
    assert!(end < disk_written, "{end} {disk_written}");
    let stored: (i64, String, i64, String) = db
        .query_row(
            "SELECT footprint_start_bytes,footprint_start_completeness,footprint_end_bytes,
                    footprint_end_completeness FROM jobs WHERE id=?1",
            [&job.job],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        stored,
        (
            job.before as i64,
            "complete".into(),
            end as i64,
            "complete".into()
        )
    );
}

/// Exercise a committed lifecycle completion and its replay against the
/// daemon after the observer's directory has actually changed.
pub fn assert_completion_replay_keeps_original_footprint(
    f: &Fixture,
    s: &Sandbox,
    db: &Connection,
) {
    use arc_ipc::jobs::{
        CompleteRequest, JobId, Outcome, ProcessIdentity, StartRequest, OPERATION_JOB_COMPLETE,
        OPERATION_JOB_START,
    };
    use serde_json::json;
    struct Restore(PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.0).unwrap();
        }
    }
    let directory = s.project("replay");
    support::write_synced(&directory.join("first"), MIB);
    let _restore = Restore(std::env::current_dir().unwrap());
    std::env::set_current_dir(&directory).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let job_id = JobId::try_from(arc_core::process_identity::random_job_id().unwrap()).unwrap();
    let call = |op: &str, payload: Value| {
        let mut request = arc_ipc::Request::new("footprint-replay", op);
        request.payload = payload;
        let response = arc_ipc::exchange(&mut f.connect(), &request).unwrap();
        assert_eq!(response.status, "ok", "{response:?}");
        response.result.unwrap()
    };
    let classification = serde_json::from_value(json!({"classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNRESOLVED_COMMAND","uncertainty":"OBSERVE"})).unwrap();
    let register = StartRequest::Register {
        job_id: job_id.clone(),
        observer: observer.clone(),
        classification,
        usage: None,
    };
    assert_eq!(
        call(OPERATION_JOB_START, json!(register))["disposition"],
        "CREATED"
    );
    let claim = StartRequest::Claim {
        job_id: job_id.clone(),
        observer: observer.clone(),
    };
    assert_eq!(
        call(OPERATION_JOB_START, json!(claim))["disposition"],
        "APPLIED"
    );
    let complete = CompleteRequest {
        job_id: job_id.clone(),
        observer,
        outcome: Outcome::Exited { code: 0 },
    };
    let original = support::allocated(&directory);
    assert_eq!(
        call(OPERATION_JOB_COMPLETE, json!(complete))["disposition"],
        "APPLIED"
    );
    support::write_synced(&directory.join("later"), 2 * MIB);
    assert!(support::allocated(&directory) > original);
    assert_eq!(
        call(OPERATION_JOB_COMPLETE, json!(complete))["disposition"],
        "UNCHANGED"
    );
    let stored: i64 = db
        .query_row(
            "SELECT footprint_end_bytes FROM jobs WHERE id=?1",
            [job_id.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, original as i64);
    let report = support::arcctl_json(
        f,
        s,
        &directory,
        &[
            "meter",
            "summary",
            "--job",
            job_id.as_str(),
            "--footprint",
            "--json",
        ],
    );
    assert_eq!(report["footprint"]["end_bytes"]["value"], original);
}

fn project_key(db: &Connection, job: &str) -> String {
    db.query_row("SELECT project_key FROM jobs WHERE id=?1", [job], |r| {
        r.get(0)
    })
    .unwrap()
}

fn tree(db: &Connection, job: &str) -> Value {
    let json: String = db
        .query_row(
            "SELECT process_tree_json FROM job_meter_summaries WHERE job_id=?1",
            [job],
            |r| r.get(0),
        )
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

/// F3, F4, MTR-015 item 3 and R-005 item 3 on the negotiated summary.
pub fn assert_cpu_quality(
    f: &Fixture,
    s: &Sandbox,
    db: &Connection,
    range: (u64, u64),
    sampled: &FootprintJob,
    tree_job: &TreeJob,
    unsampled: &WaitedOnlyJob,
) {
    let by_project = support::summary(f, s, &sampled.project, "project", range);
    support::assert_all_flagged(&by_project);

    // F4 on a real job: one sampled process, nothing unwaited, yet its waited
    // usage exceeds what the samples saw of it, so it is partial.
    let report = tree(db, &sampled.job);
    let waited = report["waited_cpu_core_seconds"]["value"].as_f64().unwrap();
    let root = report["root_sampled_cpu_core_seconds"]["value"]
        .as_f64()
        .unwrap();
    assert_eq!(report["waited_cpu_core_seconds"]["basis"], "waited_rusage");
    assert_eq!(report["processes"], 1, "{report}");
    assert_eq!(
        report["unwaited_cpu_core_seconds"]["value"], 0.0,
        "{report}"
    );
    assert!(
        waited > root + 1e-6,
        "waited {waited} vs sampled root {root}"
    );
    assert_eq!(report["completeness"], "partial", "{report}");
    assert_eq!(report["missing_coverage"][0], "unobserved_descendants");

    // F4: the root-only job has a partial tree and a sampled summary.
    let local = &support::group(&by_project, "project", &project_key(db, &sampled.job))["local"];
    let cpu = &local["cpu_core_seconds"];
    let tree_cpu = report["cpu_core_seconds"]["value"].as_f64();
    assert_eq!(cpu["value"].as_f64(), tree_cpu);
    assert_eq!(cpu["basis"], "tree_samples", "{cpu}");
    assert_eq!(cpu["availability"], "partial", "{cpu}");
    assert_eq!(
        (
            cpu["tree_sampled_jobs"].as_u64(),
            cpu["waited_only_jobs"].as_u64()
        ),
        (Some(1), Some(0))
    );
    let disk = &local["disk_write_bytes"];
    assert!(disk["value"].as_u64().unwrap() >= BIG, "{disk}");
    assert_eq!(disk["availability"], "partial", "{disk}");
    let delta = &local["footprint_delta_bytes"];
    assert_eq!(
        delta["value"].as_u64(),
        Some(sampled.after - sampled.before)
    );
    assert_eq!(delta["availability"], "complete", "{delta}");

    // F3: a real unwaited grandchild makes tree CPU strictly greater than
    // waited CPU. A waited-only value with a tree label cannot pass.
    let tree_report = tree(db, &tree_job.job);
    let tree_cpu = tree_report["cpu_core_seconds"]["value"].as_f64().unwrap();
    let waited_cpu = tree_report["waited_cpu_core_seconds"]["value"]
        .as_f64()
        .unwrap();
    let unwaited = tree_report["unwaited_cpu_core_seconds"]["value"]
        .as_f64()
        .unwrap();
    assert!(unwaited > 0.0 && tree_cpu > waited_cpu, "{tree_report}");
    let tree_summary = &support::group(&by_project, "project", &project_key(db, &tree_job.job))
        ["local"]["cpu_core_seconds"];
    assert_eq!(tree_summary["value"].as_f64(), Some(tree_cpu));
    assert_eq!(tree_summary["basis"], "tree_samples");
    assert_eq!(tree_summary["tree_sampled_jobs"], 1);

    // F3: a job no sample saw reports its waited CPU as a partial lower
    // bound, and its unsampled values are unavailable, never 0.
    let waited_report: f64 = db
        .query_row(
            "SELECT json_extract(report_json,'$.cpu_core_seconds.total') FROM job_meter_summaries
             WHERE job_id=?1",
            [&unsampled.job],
            |r| r.get(0),
        )
        .unwrap();
    let local = &support::group(&by_project, "project", &project_key(db, &unsampled.job))["local"];
    let cpu = &local["cpu_core_seconds"];
    assert_eq!(cpu["value"].as_f64(), Some(waited_report));
    assert_eq!(cpu["basis"], "waited_rusage", "{cpu}");
    assert_eq!(cpu["availability"], "partial", "{cpu}");
    assert_eq!(cpu["waited_only_jobs"], 1, "{cpu}");
    for metric in ["memory_peak_bytes", "disk_read_bytes", "disk_write_bytes"] {
        let value = &local[metric];
        assert_eq!(value["value"], Value::Null, "{metric}: {value}");
        assert_eq!(value["availability"], "unavailable", "{metric}");
        assert_eq!(value["reason"], "not_sampled", "{metric}");
    }

    // A day bucket contains both real jobs, so a measured-plus-missing
    // memory contribution must be represented as partial, never skipped.
    let registered = |job: &str| -> u64 {
        db.query_row(
            "SELECT registered_at_ms FROM job_lifecycle WHERE job_id=?1",
            [job],
            |r| r.get::<_, i64>(0),
        )
        .unwrap() as u64
    };
    let day = registered(&tree_job.job) / DAY_MS * DAY_MS;
    assert_eq!(
        registered(&unsampled.job) / DAY_MS * DAY_MS,
        day,
        "fixture crossed a day bucket"
    );
    let day_from = day.to_string();
    let day_to = (day + DAY_MS).to_string();
    let by_period = support::arcctl_json(
        f,
        s,
        &unsampled.project,
        &[
            "meter", "summary", "--by", "period", "--from", &day_from, "--to", &day_to, "--bucket",
            "day", "--json",
        ],
    );
    support::assert_all_flagged(&by_period);
    let group = by_period["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["bucket_start_ms"] == day)
        .unwrap();
    let cpu = &group["local"]["cpu_core_seconds"];
    assert_eq!(cpu["basis"], "tree_samples_and_waited_rusage", "{cpu}");
    assert_eq!(cpu["availability"], "partial", "{cpu}");
    let (eligible, measured, peak): (i64, i64, Option<i64>) = db.query_row(
        "SELECT count(CASE WHEN COALESCE(m.relation,'unverified')!='included_in_root' THEN 1 END),
                count(CASE WHEN m.relation='canonical_root' THEN json_extract(m.process_tree_json,'$.memory_peak_bytes.value') END),
                max(CASE WHEN m.relation='canonical_root' THEN json_extract(m.process_tree_json,'$.memory_peak_bytes.value') END)
         FROM job_lifecycle l JOIN jobs j ON j.id=l.job_id LEFT JOIN job_meter_summaries m ON m.job_id=j.id
         WHERE l.registered_at_ms>=?1 AND l.registered_at_ms<?2 AND j.job_class NOT IN ('REMOTE_CI','REMOTE_EXEC')",
        [day as i64, (day + DAY_MS) as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    ).unwrap();
    assert!(
        measured > 0 && measured < eligible,
        "measured {measured}, eligible {eligible}"
    );
    let peak_metric = &group["local"]["memory_peak_bytes"];
    assert_eq!(peak_metric["value"].as_i64(), peak);
    assert_eq!(peak_metric["measured_jobs"], measured);
    assert_eq!(peak_metric["eligible_jobs"], eligible);
    assert_eq!(peak_metric["availability"], "partial");
    let by_vendor = support::summary(f, s, &unsampled.project, "vendor", range);
    support::assert_all_flagged(&by_vendor);
}
