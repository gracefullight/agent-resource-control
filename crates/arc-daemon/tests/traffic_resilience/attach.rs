//! Follow-up 1: what the user is told about a child that exits at once.
//!
//! The wrapper records its child (launch acknowledgement and attach) after
//! the spawn, so a short-lived child can be gone before it is recorded. The
//! user-side rule decides whether that is a defect: with a healthy daemon and
//! a child that exited normally, stderr must not carry `TRACKING_UNCERTAIN`
//! and `arcctl job status <ID>` must say succeeded or failed with the exit
//! code, never an unknown outcome.
//!
//! Three shapes are run through built `arcctl exec` and a real daemon:
//! - the child is known to have exited before the wrapper records it. The
//!   wrapper's delay-only `spawned` checkpoint holds it after the spawn until
//!   this test has read from the process table that the child is no longer
//!   running; nothing else is injected;
//! - the same commands with nothing held, at whatever the timing is;
//! - the same commands queued behind a held permit, each starting when the
//!   one before it released the Build window (the project fairness drain).
use crate::{
    fixture::{file, World},
    journey::{eventually, queue_state, starts},
    runtime::child_watchdog::Owned,
    support,
};
use arc_core::process_identity::{probe, probe_live, IdentityStatus, ProcessIdentity};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    path::Path,
    process::{Command, Stdio},
};

/// What both user surfaces and the internal column say about one finished
/// job of the wrapper process `wrapper`.
fn reported(w: &World, db: &rusqlite::Connection, wrapper: u32, code: i32) -> bool {
    let (id, recorded, permit): (String, bool, String) = db
        .query_row(
            "SELECT l.job_id,l.child_pid IS NOT NULL,p.state FROM job_lifecycle l \
             JOIN permits p ON p.job_id=l.job_id WHERE l.observer_pid=?1",
            [wrapper],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(permit, "RELEASED");
    let status = w.status(&id);
    let expected = format!(
        "Job: {id}\nState: {}\nExit code: {code}\n",
        if code == 0 { "succeeded" } else { "failed" }
    );
    assert!(status.starts_with(&expected), "{status}");
    recorded
}

/// `arcctl exec -- cargo build` whose stub exits at once: 0 in `success`
/// mode, 17 in `echo` mode. Both are normal exits.
fn build(w: &World, mode: &str, marker: &Path) -> Command {
    let mut command = w.s.arcctl(&w.f, &w.project);
    command
        .args(["exec", "--", "cargo", "build"])
        .env("ARC_FIXTURE_MODE", mode)
        .env("ARC_FIXTURE_MARKER", marker);
    command
}

/// Run one build whose wrapper is held after its spawn until the child has
/// exited, and return its exit code and diagnostics.
fn recorded_after_exit(w: &World, mode: &str, marker: &Path, name: &str) -> (u32, i32, String) {
    let checkpoint = w.f.root.join(format!("{name}.checkpoint"));
    let stderr = w.f.root.join(format!("{name}.stderr"));
    let listener = UnixListener::bind(&checkpoint).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut command = build(w, mode, marker);
    command
        .env("ARC_PRESSURE_ACCEPTANCE_STAGE", "spawned")
        .env("ARC_PRESSURE_ACCEPTANCE_BARRIER", &checkpoint)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&stderr).unwrap());
    let mut wrapper = Owned::spawn(&mut command).unwrap();
    let mut held = None;
    eventually("the wrapper spawned its child", || {
        held = listener.accept().ok();
        held.is_some()
    });
    let (mut held, _) = held.unwrap();
    held.set_nonblocking(false).unwrap();
    held.set_read_timeout(Some(std::time::Duration::from_secs(60)))
        .unwrap();
    let mut spawned = String::new();
    BufReader::new(&held).read_line(&mut spawned).unwrap();
    let spawned: serde_json::Value = serde_json::from_str(&spawned).unwrap();
    let pid = u32::try_from(spawned["child_pid"].as_u64().unwrap()).unwrap();
    // The child has exited and is not yet reaped: its identity is still
    // there, but it is no longer a running process.
    let child = ProcessIdentity::capture(pid).unwrap();
    eventually("the child exited before it was recorded", || {
        probe_live(&child) != IdentityStatus::VerifiedMatch
    });
    assert_eq!(probe(&child), IdentityStatus::VerifiedMatch);
    assert_eq!(starts(marker), 1);
    held.write_all(b"x").unwrap();
    let status = wrapper.wait("held wrapper").unwrap();
    (wrapper.child.id(), status.code().unwrap(), file(&stderr))
}

#[test]
fn short_lived_child_attach_is_reported_truthfully() {
    const UNHELD: usize = 4;
    const QUEUED: usize = 2;
    let _serial = support::serial();
    let mut w = World::new();
    let db = w.db();
    let mut unrecorded = 0;

    // The child is gone before the wrapper records it.
    for (name, mode, code) in [("held-0", "success", 0), ("held-17", "echo", 17)] {
        let marker = w.f.root.join(format!("{name}.starts"));
        let (wrapper, exit, stderr) = recorded_after_exit(&w, mode, &marker, name);
        assert_eq!(exit, code, "{name}: {stderr}");
        // Only the child's own line: no notice of any kind from the wrapper.
        assert_eq!(stderr, "fixture-stderr\n", "{name}");
        unrecorded += usize::from(!reported(&w, &db, wrapper, code));
        assert_eq!(starts(&marker), 1, "{name}");
    }

    // Nothing held.
    let marker = w.f.root.join("unheld.starts");
    for run in 0..UNHELD {
        let (mode, code) = if run % 2 == 0 {
            ("success", 0)
        } else {
            ("echo", 17)
        };
        let mut command = build(&w, mode, &marker);
        let mut wrapper =
            Owned::spawn(command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(
                std::fs::File::create(w.f.root.join(format!("unheld-{run}.stderr"))).unwrap(),
            ))
            .unwrap();
        let status = wrapper.wait("short-lived build").unwrap();
        let stderr = file(&w.f.root.join(format!("unheld-{run}.stderr")));
        assert_eq!(status.code(), Some(code), "run {run}: {stderr}");
        assert_eq!(stderr, "fixture-stderr\n", "run {run}");
        unrecorded += usize::from(!reported(&w, &db, wrapper.child.id(), code));
    }
    assert_eq!(starts(&marker), UNHELD);

    // Queued behind a held permit.
    let mut holder = w.start("build");
    holder.running(&db);
    let marker = w.f.root.join("queued.starts");
    let mut queued = Vec::new();
    for index in 0..QUEUED {
        let mut command = build(&w, "success", &marker);
        let wrapper = Owned::spawn(command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(
            std::fs::File::create(w.f.root.join(format!("queued-{index}.stderr"))).unwrap(),
        ))
        .unwrap();
        let pid = wrapper.child.id();
        eventually("short-lived build queued", || {
            queue_state(&db, pid).as_deref() == Some("QUEUED")
        });
        queued.push(wrapper);
    }
    holder.release();
    assert!(holder.wrapper.wait("holder").unwrap().success());
    for (index, wrapper) in queued.iter_mut().enumerate() {
        let status = wrapper.wait("queued short-lived build").unwrap();
        let stderr = file(&w.f.root.join(format!("queued-{index}.stderr")));
        assert!(status.success(), "queued {index}: {status:?} {stderr}");
        // A waiter is told that it waits; nothing says tracking is in doubt.
        assert!(stderr.starts_with("QUEUED\n"), "queued {index}: {stderr}");
        assert!(
            stderr.ends_with("fixture-stderr\n")
                && !stderr.contains("UNCERTAIN")
                && !stderr.contains("UNCONFIRMED"),
            "queued {index}: {stderr}"
        );
        unrecorded += usize::from(!reported(&w, &db, wrapper.child.id(), 0));
    }
    assert_eq!(starts(&marker), QUEUED);
    let active: i64 = db
        .query_row(
            "SELECT count(*) FROM permits WHERE state='ACTIVE'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(active, 0);
    // Printed for the record, never asserted: the internal column alone.
    println!(
        "SHORT_LIVED_ATTACH {{\"runs\":{},\"child_pid_null\":{unrecorded}}}",
        2 + UNHELD + QUEUED
    );
    drop(db);
    w.finish();
}
