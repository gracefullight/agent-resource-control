//! Native unwaited CPU consumed entirely between observed daemon samples.
use crate::{
    journey, quality_support as support, runtime::child_watchdog, tree_fixture, Fixture, Sandbox,
};
use arc_core::process_identity::{probe_live, IdentityStatus};
use rusqlite::Connection;
use std::{collections::BTreeSet, fs, process::Stdio};

fn stopped(pid: u32) -> bool {
    // SAFETY: exact native output buffer for a fixture-owned process.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&info) as libc::c_int;
    let result = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            std::ptr::from_mut(&mut info).cast(),
            size,
        )
    };
    result == size && info.pbi_status == libc::SSTOP
}

pub fn missed_unwaited_child_stays_partial(f: &Fixture, s: &Sandbox, db: &Connection) {
    const ATTEMPTS: usize = 3;
    let mut missed_any = false;
    for attempt in 0..ATTEMPTS {
        let project = s.project(&format!("cpu-gap-{attempt}"));
        let control = support::directory(f, &format!("cpu-gap-control-{attempt}"));
        let mut command = s.arcctl(f, &project);
        command
            .args(["exec", "--", support::PROGRAM, "cpu-miss"])
            .arg(&control)
            .stdin(Stdio::null());
        let mut exec = child_watchdog::Owned::spawn(&mut command).unwrap();
        let root = support::published(&control, "cpu-root.pid");
        let root_owner = tree_fixture::Reaped::capture(root);
        let job = support::running_job(db, root);
        let mut before = BTreeSet::new();
        journey::eventually("two root samples before the short-lived child", || {
            before = tree_fixture::rows(db, &job)
                .into_iter()
                .map(|r| r.timestamp_ns)
                .collect();
            before.len() >= 2
        });
        fs::write(control.join("fork"), b"").unwrap();
        let child = support::published(&control, "cpu-child.pid");
        let owned = tree_fixture::Reaped::capture(child);
        fs::write(control.join("burn"), b"").unwrap();
        let child_cpu_us = support::published(&control, "cpu-child-us");
        assert!(child_cpu_us >= 20_000);
        support::published(&control, "cpu-ready");
        journey::eventually("the unwaited CPU burner has exited", || {
            probe_live(&owned.0) != IdentityStatus::VerifiedMatch
        });
        journey::eventually("the root has stopped before its final sample", || {
            stopped(root as u32)
                && stopped(exec.child.id())
                && probe_live(&root_owner.0) == IdentityStatus::VerifiedMatch
        });
        let stopped_samples: BTreeSet<_> = tree_fixture::rows(db, &job)
            .iter()
            .map(|r| r.timestamp_ns)
            .collect();
        journey::eventually("root sampled after the unwaited child exited", || {
            tree_fixture::rows(db, &job)
                .iter()
                .any(|r| !stopped_samples.contains(&r.timestamp_ns))
        });
        let missed = tree_fixture::rows(db, &job)
            .iter()
            .all(|r| u64::from(r.identity.pid) != child);
        // The test owns and identity-checks the stopped fixture root.
        assert_eq!(probe_live(&root_owner.0), IdentityStatus::VerifiedMatch);
        assert_eq!(unsafe { libc::kill(root as i32, libc::SIGKILL) }, 0);
        // arcctl mirrors its child's stop; resume our owned wrapper to reap it.
        assert_eq!(
            unsafe { libc::kill(exec.child.id() as i32, libc::SIGCONT) },
            0
        );
        assert!(!exec
            .wait("short-lived unwaited CPU fixture")
            .unwrap()
            .success());
        if !missed {
            eprintln!("CPU-gap attempt {attempt}: a short-lived child was sampled");
            continue;
        }
        missed_any = true;
        journey::eventually("finalized process tree and waited usage", || {
            db.query_row("SELECT json_extract(process_tree_json,'$.terminal')=1 AND json_extract(process_tree_json,'$.waited_cpu_core_seconds.value') IS NOT NULL FROM job_meter_summaries WHERE job_id=?1", [&job], |r| r.get::<_, Option<bool>>(0)).ok().flatten().unwrap_or(false)
        });
        let report = support::arcctl_json(
            f,
            s,
            &project,
            &["meter", "summary", "--job", &job, "--quality", "--json"],
        );
        let tree = &report["process_tree"];
        let waited = tree["waited_cpu_core_seconds"]["value"].as_f64().unwrap();
        let root_sample = tree["root_sampled_cpu_core_seconds"]["value"]
            .as_f64()
            .unwrap();
        // Require the exact legacy false-complete condition, not merely a
        // missed child whose already-partial legacy tree masks the defect.
        let false_complete = tree["completeness"] == "complete" && waited <= root_sample;
        if !false_complete {
            eprintln!("CPU-gap attempt {attempt}: legacy {:?}, waited {waited}, sampled root {root_sample}", tree["completeness"]);
            continue;
        }
        assert_eq!(tree["processes"], 1, "{tree}");
        for metric in ["cpu_core_seconds", "unwaited_cpu_core_seconds"] {
            let value = &report["process_tree_quality"][metric];
            support::assert_flagged(metric, value);
            assert_eq!(value["availability"], "partial", "{report}");
            assert_eq!(value["value"], tree[metric]["value"]);
        }
        assert_eq!(tree["unwaited_cpu_core_seconds"]["value"], 0.0);
        let (registered, key): (i64, String) = db.query_row("SELECT l.registered_at_ms,j.project_key FROM job_lifecycle l JOIN jobs j ON j.id=l.job_id WHERE j.id=?1", [&job], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        let hour = registered as u64 / 3_600_000 * 3_600_000;
        let summary = support::summary(f, s, &project, "project", (hour, hour + 3_600_000));
        let cpu = &support::group(&summary, "project", &key)["local"]["cpu_core_seconds"];
        assert_eq!(cpu["measured_jobs"], 1, "{summary}");
        assert_eq!(cpu["availability"], "partial", "{summary}");
        assert_eq!(cpu["value"], tree["cpu_core_seconds"]["value"]);
        eprintln!("missed unwaited child: {child_cpu_us} us CPU, legacy tree {}, waited {waited}, sampled root {root_sample}; individual and project CPU are partial", tree["completeness"]);
        return;
    }
    panic!("no native run produced an unwaited CPU burner missed by every daemon sample (missed in some attempt: {missed_any})");
}
