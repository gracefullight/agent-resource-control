//! Native fixture observation before and after a committed permit release.
use crate::{
    fixture,
    journey::{self, Sandbox},
    runtime::{child_watchdog, Fixture},
    summary::{identity, proven},
    tree,
};
use arc_core::process_identity::{probe_live, IdentityStatus};
use rusqlite::Connection;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

pub fn start(
    f: &Fixture,
    s: &Sandbox,
    project: &Path,
    db: &Connection,
    lifeline: &fixture::Lifeline,
) -> (PathBuf, child_watchdog::Owned, String) {
    let directory = tree::directory(f, "managed-2");
    let mut exec = s.arcctl(f, project);
    lifeline
        .arm(&mut exec)
        .args(["exec", "--", fixture::PROGRAM, "build"])
        .env("ARC_ATTRIBUTION_DIR", &directory)
        .env("ARC_ATTRIBUTION_DETACH_CHILD", "1")
        .env("ARC_ATTRIBUTION_SELF_OBSERVE", f.root.join("self2.sock"))
        .stdin(Stdio::null());
    let second = child_watchdog::Owned::spawn(&mut exec).unwrap();
    let root = tree::published(&directory, "root.pid");
    let job = tree::running_job(db, root);
    (directory, second, job)
}

pub struct Context<'a> {
    pub f: &'a Fixture,
    pub s: &'a Sandbox,
    pub db: &'a Connection,
    pub project: &'a Path,
    pub programs: &'a fixture::Programs,
    pub lifeline: &'a fixture::Lifeline,
    pub directory: &'a Path,
    pub job: &'a str,
    pub second: &'a mut child_watchdog::Owned,
}

pub fn check(c: Context<'_>) -> f64 {
    let child = identity(tree::published(c.directory, "child.pid"));
    let _reaped = tree::Reaped(child.clone());
    let request = fixture::observation(c.directory, "child");
    journey::eventually("managed child observed as Proven", || {
        fixture::probe(&request).is_some_and(|p| proven(&p) > 0.0)
    });
    // Keep the cadence active while the surviving descendant is scanned.
    let keep_dir = tree::directory(c.f, "active-after-release");
    let mut keep = c.s.arcctl(c.f, c.project);
    c.lifeline
        .arm(&mut keep)
        .arg("exec")
        .arg("--")
        .arg(&c.programs.burner)
        .args(["burn", "keep-active"])
        .arg(&keep_dir)
        .env("ARC_ATTRIBUTION_SELF_OBSERVE", c.f.root.join("self2.sock"))
        .stdin(Stdio::null());
    let mut keep = child_watchdog::Owned::spawn(&mut keep).unwrap();
    let keep_pid = tree::published(&keep_dir, "keep-active.pid");
    let _keep_job = tree::running_job(c.db, keep_pid);
    tree::release(c.directory);
    assert!(c.second.wait("second managed job").unwrap().success());
    let (released, released_at): (String, i64) =
        c.db.query_row(
            "SELECT state,released_at FROM permits WHERE job_id=?1",
            [c.job],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(released, "RELEASED");
    let released_at = u64::try_from(released_at).unwrap();
    journey::eventually(
        "surviving child observed after its lease was released",
        || {
            fixture::probe(&request).is_some_and(|p| {
                p.lines.iter().any(|line| {
                    matches!(line,
                fixture::Line::Counted { confidence, delta_ns, scan, .. }
                if scan.start_ms > released_at && confidence == "unattributed" && *delta_ns > 0)
                })
            })
        },
    );
    let observation = fixture::probe(&request).unwrap();
    for line in &observation.lines {
        if let fixture::Line::Counted {
            confidence, scan, ..
        } = line
        {
            if scan.start_ms > released_at {
                assert_eq!(confidence, "unattributed", "post-release scan {line:?}");
            }
        }
    }
    let child_proven = proven(&observation);
    std::fs::write(c.directory.join("child-release"), "").unwrap();
    tree::release(&keep_dir);
    assert!(keep.wait("cadence holder").unwrap().success());
    journey::eventually("released child exited", || {
        probe_live(&child) != IdentityStatus::VerifiedMatch
    });
    child_proven
}
