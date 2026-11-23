//! Integrated Traffic footprint and per-value quality checks.
use crate::{
    journey::{self, Sandbox},
    quality::{FootprintJob, WaitedOnlyJob},
    quality_support::{self as support, MIB, PROGRAM},
    runtime::{child_watchdog, Fixture},
};
use rusqlite::Connection;
use std::{fs, process::Stdio};

const BIG: u64 = 32 * MIB;
/// Every individually flagged process-tree value.
pub const TREE_KEYS: [&str; 7] = [
    "cpu_core_seconds",
    "waited_cpu_core_seconds",
    "root_sampled_cpu_core_seconds",
    "unwaited_cpu_core_seconds",
    "memory_peak_bytes",
    "disk_read_bytes",
    "disk_write_bytes",
];
/// Sampled values that are lower bounds whatever the tree's CPU completeness.
pub const SAMPLED_BOUNDS: [&str; 3] = ["memory_peak_bytes", "disk_read_bytes", "disk_write_bytes"];
const LEFT: u64 = MIB;
const EXISTING: u64 = 2 * MIB;

/// A classified BUILD waits behind another BUILD, then runs the same disk
/// fixture only after Traffic grants its permit.
pub fn traffic_footprint(f: &Fixture, s: &Sandbox, db: &Connection) -> FootprintJob {
    let bin = f.root.join("traffic-quality-bin");
    fs::create_dir(&bin).unwrap();
    fs::copy(s.bin.join(PROGRAM), bin.join("cargo")).unwrap();
    let path = std::env::join_paths([&bin, &s.bin]).unwrap();
    let held_project = s.project("traffic-held");
    let project = s.project("traffic-footprint");
    support::write_synced(&project.join("existing.bin"), EXISTING);
    let before = support::allocated(&project);
    let hold = support::directory(f, "traffic-hold-control");
    let control = support::directory(f, "traffic-footprint-control");
    let command = |cwd: &std::path::Path, mode: &str, directory: &std::path::Path| {
        let mut command = s.arcctl(f, cwd);
        command
            .args(["exec", "--", "cargo", "build"])
            .env("PATH", &path)
            .env("ARC_QUALITY_MODE", mode)
            .env("ARC_QUALITY_CONTROL", directory)
            .stdin(Stdio::null());
        command
    };
    let mut held =
        child_watchdog::Owned::spawn(&mut command(&held_project, "hold", &hold)).unwrap();
    let held_pid = support::published(&hold, "hold.pid");
    let held_job = support::running_job(db, held_pid);
    assert_running_report_is_one_snapshot(f, s, &held_project, &held_job);
    let mut queued =
        child_watchdog::Owned::spawn(&mut command(&project, "footprint", &control)).unwrap();
    let wrapper = queued.child.id();
    journey::eventually("classified build queued before launch", || {
        assert!(
            queued.child.try_wait().unwrap().is_none(),
            "queued build exited"
        );
        journey::queue_state(db, wrapper).as_deref() == Some("QUEUED")
    });
    let (job, class, start): (String, String, Option<String>) = db
        .query_row(
            "SELECT j.id,j.job_class,j.footprint_start_completeness FROM jobs j
         JOIN job_lifecycle l ON l.job_id=j.id WHERE l.observer_pid=?1",
            [wrapper],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(class, "BUILD");
    assert!(start.is_none(), "queued work has not been granted");
    assert!(
        !control.join("footprint.pid").exists(),
        "queued work did not start"
    );
    support::release(&hold);
    assert!(held.wait("held build").unwrap().success());
    let pid = support::published(&control, "footprint.pid");
    assert_eq!(support::running_job(db, pid), job);
    let written = support::published(&control, "written");
    assert_eq!(written, BIG + LEFT);
    assert_eq!(support::published(&control, "left"), LEFT);
    journey::eventually("granted build has a sample with written bytes", || {
        support::samples(db, &job).1 >= written
    });
    support::release(&control);
    assert!(queued.wait("queued footprint build").unwrap().success());
    let after = support::allocated(&project);
    FootprintJob {
        job,
        project,
        before,
        after,
        written,
    }
}

/// A paused Governor releases a classified build without a Traffic permit;
/// its committed launch and completion still bracket the footprint walks.
pub fn unreserved_footprint(f: &Fixture, s: &Sandbox, db: &Connection) -> FootprintJob {
    let project = s.project("unreserved-footprint");
    let paused = support::arcctl_json(f, s, &project, &["daemon", "pause", "--json"]);
    assert_eq!(paused["switch"]["enabled"], false);
    support::write_synced(&project.join("existing.bin"), EXISTING);
    let before = support::allocated(&project);
    let control = support::directory(f, "unreserved-footprint-control");
    let bin = f.root.join("traffic-quality-bin");
    let path = std::env::join_paths([&bin, &s.bin]).unwrap();
    let mut command = s.arcctl(f, &project);
    command
        .args(["exec", "--", "cargo", "build"])
        .env("PATH", path)
        .env("ARC_QUALITY_MODE", "footprint")
        .env("ARC_QUALITY_CONTROL", &control)
        .stdin(Stdio::null());
    let mut exec = child_watchdog::Owned::spawn(&mut command).unwrap();
    let pid = support::published(&control, "footprint.pid");
    let job = support::running_job(db, pid);
    let (mode, start): (String, Option<String>) = db.query_row(
        "SELECT g.mode,j.footprint_start_completeness FROM governor_jobs g JOIN jobs j ON j.id=g.job_id WHERE g.job_id=?1",
        [&job], |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap();
    assert_eq!(mode, "UNRESERVED");
    assert_eq!(start.as_deref(), Some("complete"));
    let written = support::published(&control, "written");
    assert_eq!(written, BIG + LEFT);
    assert_eq!(support::published(&control, "left"), LEFT);
    journey::eventually("unreserved build has a sample with written bytes", || {
        support::samples(db, &job).1 >= written
    });
    support::release(&control);
    assert!(exec.wait("unreserved footprint build").unwrap().success());
    let after = support::allocated(&project);
    let resumed = support::arcctl_json(f, s, &project, &["daemon", "resume", "--json"]);
    assert_eq!(resumed["switch"]["enabled"], true);
    FootprintJob {
        job,
        project,
        before,
        after,
        written,
    }
}

/// The separately negotiated report flags every process-tree value.
pub fn assert_tree_quality(
    f: &Fixture,
    s: &Sandbox,
    sampled: &FootprintJob,
    missing: &WaitedOnlyJob,
) {
    let read = |project: &std::path::Path, job: &str| {
        support::arcctl_json(
            f,
            s,
            project,
            &["meter", "summary", "--job", job, "--quality", "--json"],
        )
    };
    let report = read(&sampled.project, &sampled.job);
    for key in TREE_KEYS {
        let flagged = &report["process_tree_quality"][key];
        support::assert_flagged(key, flagged);
        assert_eq!(
            flagged["value"], report["process_tree"][key]["value"],
            "{key}: {report}"
        );
    }
    assert_eq!(
        report["process_tree_quality"]["cpu_core_seconds"]["availability"], "partial",
        "{report}"
    );
    for key in SAMPLED_BOUNDS {
        let flagged = &report["process_tree_quality"][key];
        assert!(flagged["value"].is_u64(), "{key}: {report}");
        assert_eq!(flagged["availability"], "partial", "{key}: {report}");
    }
    let missing = read(&missing.project, &missing.job);
    assert!(missing["process_tree"].is_null(), "{missing}");
    for key in TREE_KEYS {
        let flagged = &missing["process_tree_quality"][key];
        support::assert_flagged(key, flagged);
        assert_eq!(flagged["availability"], "unavailable", "{key}: {missing}");
        assert_eq!(flagged["reason"], "not_recorded", "{key}: {missing}");
    }
}

/// While a held build runs and the daemon keeps sampling it, every quality
/// read is one snapshot: the tree it reports is exactly the tree it flags,
/// across reads between which the sampled tree demonstrably changed.
fn assert_running_report_is_one_snapshot(
    f: &Fixture,
    s: &Sandbox,
    project: &std::path::Path,
    job: &str,
) {
    let mut samples = std::collections::BTreeSet::new();
    journey::eventually("a running job's tree changes between quality reads", || {
        let report = support::arcctl_json(
            f,
            s,
            project,
            &["meter", "summary", "--job", job, "--quality", "--json"],
        );
        let tree = &report["process_tree"];
        if tree.is_null() {
            return false;
        }
        assert_eq!(tree["terminal"], false, "{report}");
        for key in TREE_KEYS {
            let flagged = &report["process_tree_quality"][key];
            support::assert_flagged(key, flagged);
            assert_eq!(flagged["value"], tree[key]["value"], "{key}: {report}");
        }
        samples.insert(tree["samples"].as_u64().unwrap());
        samples.len() >= 3
    });
}

/// A Traffic grant and completion each take their reading only in the writer
/// call whose own transaction committed them. With the reading the original
/// attempt failed to store (the persisted state such a failure leaves: the
/// reading never written), a replayed grant and a replayed completion after
/// the directory grew leave it unavailable instead of recording the later
/// state as the original measurement.
pub fn assert_traffic_replay_keeps_original_footprint(f: &Fixture, s: &Sandbox, db: &Connection) {
    use arc_ipc::{
        jobs::{JobId, Outcome, ProcessIdentity},
        traffic::{JobRequest, OPERATION_TRAFFIC_JOB},
    };
    use serde_json::{json, Value};
    struct Restore(std::path::PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.0).unwrap();
        }
    }
    let directory = s.project("traffic-replay");
    support::write_synced(&directory.join("first"), MIB);
    let _restore = Restore(std::env::current_dir().unwrap());
    std::env::set_current_dir(&directory).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let job_id = JobId::try_from(arc_core::process_identity::random_job_id().unwrap()).unwrap();
    let call = |job: &JobRequest| -> Value {
        let mut request = arc_ipc::Request::new("traffic-footprint-replay", OPERATION_TRAFFIC_JOB);
        request.payload = serde_json::to_value(job).unwrap();
        let response = arc_ipc::exchange(&mut f.connect(), &request).unwrap();
        assert_eq!(response.status, "ok", "{response:?}");
        response.result.unwrap()
    };
    let readings = || -> (Option<i64>, Option<String>, Option<i64>, Option<String>) {
        db.query_row(
            "SELECT footprint_start_bytes,footprint_start_completeness,footprint_end_bytes,
                    footprint_end_completeness FROM jobs WHERE id=?1",
            [job_id.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap()
    };
    let classification = serde_json::from_value(json!({"classification":"BUILD",
        "locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"}))
    .unwrap();
    let granted = call(&JobRequest::Enqueue {
        job_id: job_id.clone(),
        observer: observer.clone(),
        classification,
        lineage_context: None,
        project: None,
        event: None,
        usage: None,
    });
    assert_eq!(granted["state"], "GRANTED", "{granted}");
    let at_grant = support::allocated(&directory);
    let (start, start_c, _, _) = readings();
    assert_eq!(
        (start, start_c.as_deref()),
        (Some(at_grant as i64), Some("complete"))
    );
    // The original grant's reading was lost; a replayed grant after the
    // directory changed must not stand in for it.
    db.execute(
        "UPDATE jobs SET footprint_start_bytes=NULL,footprint_start_completeness=NULL WHERE id=?1",
        [job_id.as_str()],
    )
    .unwrap();
    support::write_synced(&directory.join("after-grant"), MIB);
    assert!(support::allocated(&directory) > at_grant);
    let replayed = call(&JobRequest::Poll {
        job_id: job_id.clone(),
        observer: observer.clone(),
    });
    assert_eq!(replayed["state"], "GRANTED", "{replayed}");
    assert_eq!(readings().1, None, "a replayed grant took a reading");

    let complete = JobRequest::Complete {
        job_id: job_id.clone(),
        observer,
        outcome: Outcome::Exited { code: 0 },
        permit_generation: granted["permit_generation"].as_u64().unwrap(),
    };
    assert_eq!(call(&complete)["state"], "COMPLETED");
    let at_end = support::allocated(&directory);
    let (_, _, end, end_c) = readings();
    assert_eq!(
        (end, end_c.as_deref()),
        (Some(at_end as i64), Some("complete"))
    );
    db.execute(
        "UPDATE jobs SET footprint_end_bytes=NULL,footprint_end_completeness=NULL WHERE id=?1",
        [job_id.as_str()],
    )
    .unwrap();
    support::write_synced(&directory.join("after-completion"), 2 * MIB);
    assert!(support::allocated(&directory) > at_end);
    assert_eq!(call(&complete)["state"], "COMPLETED", "replayed completion");
    assert_eq!(readings(), (None, None, None, None));
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
    for key in ["start_bytes", "end_bytes", "delta_bytes"] {
        let value = &report["footprint"][key];
        support::assert_flagged(key, value);
        assert_eq!(value["availability"], "unavailable", "{key}: {report}");
        assert_eq!(value["reason"], "not_measured", "{key}: {report}");
    }
}

/// A real missed sample: a short-lived child writes 4 MiB and is reaped
/// before any sample of the job saw it (checked in the daemon's own samples;
/// a run whose child was sampled is retried). The sampled disk total is then
/// below what the job wrote, so the built CLI's per-value report and the v2
/// aggregate label it, and the other sampled bounds, partial, never complete.
pub fn assert_missed_sample_is_partial(f: &Fixture, s: &Sandbox, db: &Connection) {
    const SPIKE: u64 = 4 * MIB;
    for attempt in 0..10 {
        let project = s.project(&format!("missed-sample-{attempt}"));
        let control = support::directory(f, &format!("missed-sample-control-{attempt}"));
        let mut command = s.arcctl(f, &project);
        command
            .args(["exec", "--", PROGRAM, "spike"])
            .arg(&control)
            .stdin(Stdio::null());
        let mut exec = child_watchdog::Owned::spawn(&mut command).unwrap();
        let pid = support::published(&control, "spike.pid");
        let child = support::published(&control, "spike-child.pid");
        let job = support::running_job(db, pid);
        journey::eventually("two samples of the job after its child exited", || {
            let stamps: std::collections::BTreeSet<_> = crate::tree_fixture::rows(db, &job)
                .iter()
                .map(|r| r.timestamp_ns)
                .collect();
            stamps.len() >= 2
        });
        let missed = crate::tree_fixture::rows(db, &job)
            .iter()
            .all(|r| u64::from(r.identity.pid) != child);
        support::release(&control);
        assert!(exec.wait("missed-sample job").unwrap().success());
        if !missed {
            continue;
        }
        let report = support::arcctl_json(
            f,
            s,
            &project,
            &["meter", "summary", "--job", &job, "--quality", "--json"],
        );
        let written = &report["process_tree_quality"]["disk_write_bytes"];
        assert!(written["value"].as_u64().unwrap() < SPIKE, "{report}");
        for key in SAMPLED_BOUNDS {
            let flagged = &report["process_tree_quality"][key];
            support::assert_flagged(key, flagged);
            assert_eq!(flagged["availability"], "partial", "{key}: {report}");
            assert_eq!(flagged["value"], report["process_tree"][key]["value"]);
        }
        let registered: i64 = db
            .query_row(
                "SELECT registered_at_ms FROM job_lifecycle WHERE job_id=?1",
                [&job],
                |r| r.get(0),
            )
            .unwrap();
        let hour = registered as u64 / 3_600_000 * 3_600_000;
        let summary = support::summary(f, s, &project, "project", (hour, hour + 3_600_000));
        support::assert_all_flagged(&summary);
        let key: String = db
            .query_row("SELECT project_key FROM jobs WHERE id=?1", [&job], |r| {
                r.get(0)
            })
            .unwrap();
        let local = &support::group(&summary, "project", &key)["local"];
        assert_eq!(local["disk_write_bytes"]["measured_jobs"], 1, "{local}");
        assert!(local["disk_write_bytes"]["value"].as_u64().unwrap() < SPIKE);
        for metric in SAMPLED_BOUNDS {
            assert_eq!(
                local[metric]["availability"], "partial",
                "{metric}: {local}"
            );
        }
        return;
    }
    panic!("every spike child was sampled");
}
