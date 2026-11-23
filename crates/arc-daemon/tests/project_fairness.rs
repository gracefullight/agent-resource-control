//! Project fair queuing through built `arcctl exec` and a real daemon
//! (AT-TRF-08). Tools are offline stubs; the Build window is 1.
#[path = "support/journey.rs"]
mod journey;
#[allow(dead_code)] // Only the admission barrier is used here.
#[path = "support/native_exec.rs"]
mod native_exec;
#[allow(dead_code)] // The shared fixture also serves unrelated workloads.
#[path = "support/runtime.rs"]
mod runtime;
mod support;

use journey::{eventually, output_with_input, queue_state, starts, Sandbox};
use runtime::{child_watchdog::Owned, Fixture};
use serde_json::Value;
use std::{
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Instant,
};

fn diagnostics(marker: &Path, name: &str) -> PathBuf {
    marker.with_extension(format!("{name}.stderr"))
}

fn wait_queued(db: &rusqlite::Connection, waiter: &mut Owned, marker: &Path, name: &str) {
    let pid = waiter.child.id();
    eventually(&format!("{name} queued"), || {
        if let Some(status) = waiter.child.try_wait().unwrap() {
            panic!(
                "{name} left before it was queued: {status:?}, queue state {:?}, {} child starts, stderr {:?}",
                queue_state(db, pid), starts(marker), std::fs::read_to_string(diagnostics(marker, name))
            );
        }
        queue_state(db, pid).as_deref() == Some("QUEUED")
    });
}

fn assert_queued(db: &rusqlite::Connection, waiter: &mut Owned, marker: &Path, name: &str) {
    assert!(
        waiter.child.try_wait().unwrap().is_none(),
        "{name} left the queue: {:?}",
        std::fs::read_to_string(diagnostics(marker, name))
    );
    let pid = waiter.child.id();
    assert_eq!(queue_state(db, pid).as_deref(), Some("QUEUED"));
}

fn counts(db: &rusqlite::Connection) -> (i64, i64, i64) {
    db.query_row(
        "SELECT (SELECT count(*) FROM traffic_entries WHERE state='QUEUED'), \
         (SELECT count(*) FROM permits WHERE state='ACTIVE'), \
         (SELECT count(*) FROM permits)",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap()
}

fn state_count(db: &rusqlite::Connection, state: &str) -> i64 {
    db.query_row(
        "SELECT count(*) FROM traffic_entries WHERE state=?1",
        [state],
        |r| r.get(0),
    )
    .unwrap()
}

fn report(f: &Fixture, label: &str, mut details: Value) {
    let report = f.report();
    let metrics = &report["metrics"];
    for key in ["connections", "requests", "db_queued"] {
        details[format!("{key}_high_water")] = metrics[key]["high_water"].clone();
    }
    details["frames_completed"] = metrics["frames_completed"].clone();
    println!("{label} {details}");
    assert_eq!(report["profiles"]["ipc"], "default");
    for key in ["connection_rejected", "request_rejected", "db_rejected"] {
        assert_eq!(metrics[key], 0, "{report}");
    }
}

/// Keep the child at its barrier until the daemon attaches its exact identity.
fn accept_running(
    db: &rusqlite::Connection,
    wrapper: &mut Owned,
    listener: &UnixListener,
    marker: &Path,
    name: &str,
) -> UnixStream {
    use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};

    let (control, barrier) = native_exec::accept(listener);
    control.set_nonblocking(true).unwrap();
    let wrapper_pid = wrapper.child.id();
    let child: ProcessIdentity = serde_json::from_value(barrier["identity"].clone())
        .expect("barrier must identify its child");
    assert_eq!(barrier["pid"].as_u64(), Some(u64::from(child.pid)));
    assert_eq!(barrier["ppid"].as_u64(), Some(u64::from(wrapper_pid)));
    let mut last_state = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        eventually(
            &format!("{name}: child {} attached to RUNNING", child.pid),
            || {
                type Attachment = (String, Option<u32>, Option<String>, Option<String>);
                let (state, attached_pid, attached_start, attached_boot): Attachment = db
                    .query_row(
                        "SELECT t.state,l.child_pid,l.child_start_token,l.child_boot_id FROM traffic_entries t \
                         JOIN job_lifecycle l ON l.job_id=t.job_id WHERE l.observer_pid=?1",
                        [wrapper_pid],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                    )
                    .unwrap_or_else(|error| panic!("{name}: cannot read daemon state: {error}"));
                last_state = Some((state.clone(), attached_pid));
                let status = wrapper.child.try_wait().expect("check wrapper status");
                assert!(status.is_none(), "{name}: wrapper exited early: {status:?}");
                assert_eq!(
                    probe_live(&child),
                    IdentityStatus::VerifiedMatch,
                    "{name}: held child {} is no longer verifiably live",
                    child.pid
                );
                // The fixture waits for a byte from us; reading cannot release it.
                match (&control).read(&mut [0]) {
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => return false,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    result => panic!("{name}: child barrier closed or sent data: {result:?}"),
                }
                match state.as_str() {
                    "RESERVED" => false,
                    "RUNNING" => {
                        assert_eq!(
                            attached_pid,
                            Some(child.pid),
                            "{name}: daemon attached a different child"
                        );
                        assert_eq!(attached_start.as_deref(), Some(child.start_token.as_str()));
                        assert_eq!(attached_boot.as_deref(), Some(child.boot_id.as_str()));
                        true
                    }
                    _ => panic!("{name}: invalid state before attachment: {state}"),
                }
            },
        );
    }));
    if let Err(payload) = result {
        eprintln!(
            "{name}: wrapper {wrapper_pid}, child {}, last daemon state {last_state:?}, stderr {:?}",
            child.pid,
            std::fs::read_to_string(diagnostics(marker, name))
        );
        std::panic::resume_unwind(payload);
    }
    control.set_nonblocking(false).unwrap();
    control
}

/// Run an offline build, recording child starts and wrapper diagnostics.
fn build(f: &Fixture, s: &Sandbox, project: &Path, marker: &Path, name: &str) -> Command {
    let mut command = s.arcctl(f, project);
    command
        .args(["exec", "--", "cargo", "build"])
        .env("ARC_FIXTURE_MODE", "success")
        .env("ARC_FIXTURE_MARKER", marker)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(diagnostics(marker, name)).unwrap());
    command
}

/// Grants are recorded at admission, including before child attachment.
/// With one permit, each grant must follow its predecessor's release.
fn granted_order(db: &rusqlite::Connection, of: &[u32]) -> Vec<u32> {
    let grants: Vec<(u32, i64, Option<i64>)> = db
        .prepare(
            "SELECT l.observer_pid,t.granted_at_ms,p.released_at FROM traffic_entries t \
             JOIN job_lifecycle l ON l.job_id=t.job_id JOIN permits p ON p.job_id=t.job_id \
             WHERE t.granted_at_ms IS NOT NULL ORDER BY t.granted_at_ms,t.sequence",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .filter(|(observer, _, _)| of.contains(observer))
        .collect();
    for pair in grants.windows(2) {
        assert!(
            pair[0].2.is_some_and(|released| released <= pair[1].1),
            "a build was granted before the one ahead of it released: {pair:?}"
        );
    }
    grants.into_iter().map(|(pid, _, _)| pid).collect()
}

/// Queue two builds behind one permit, then verify FIFO execution and release.
#[test]
fn seam_two_queued_builds_keep_fifo_order_through_built_arcctl() {
    let waiters = 2;
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    s.stub("cargo");
    let (held_project, project) = (s.project("held"), s.project("project-a"));
    let marker = f.root.join("waiter-starts");
    let mut daemon = f.spawn_isolated("");
    let started = Instant::now();

    // The child reaches the barrier after admission and native spawn.
    let listener = UnixListener::bind(f.root.join("held-build")).unwrap();
    let held_starts = f.root.join("held-starts");
    let mut holder = build(&f, &s, &held_project, &held_starts, "holder");
    holder.env("ARC_FIXTURE_CONTROL", f.root.join("held-build"));
    let mut holder = Owned::spawn(&mut holder).unwrap();
    let db = journey::database(&f);
    let mut release = accept_running(&db, &mut holder, &listener, &held_starts, "holder");

    let mut queued = Vec::with_capacity(waiters);
    for index in 0..waiters {
        let name = format!("waiter-{index}");
        let mut waiter = Owned::spawn(&mut build(&f, &s, &project, &marker, &name)).unwrap();
        wait_queued(&db, &mut waiter, &marker, &name);
        queued.push(waiter);
    }
    let queued_after = started.elapsed();
    let counts = || counts(&db);
    assert_eq!(counts(), (waiters as i64, 1, 1));
    assert_eq!(starts(&marker), 0, "a queued build started its child");
    for (index, waiter) in queued.iter_mut().enumerate() {
        assert_queued(&db, waiter, &marker, &format!("waiter-{index}"));
    }

    release.write_all(b"x").unwrap();
    assert!(holder.wait("held build").unwrap().success());
    for (index, waiter) in queued.iter_mut().enumerate() {
        let status = waiter.wait(&format!("waiter {index}")).unwrap();
        assert!(status.success(), "waiter {index}: {status:?}");
    }
    // A one-permit grant order is the start order.
    let expected: Vec<u32> = queued.iter().map(|waiter| waiter.child.id()).collect();
    assert_eq!(
        granted_order(&db, &expected),
        expected,
        "each build runs in queue order"
    );
    assert_eq!(starts(&marker), waiters, "each build runs exactly once");
    eventually("all permits released", || {
        counts() == (0, 0, waiters as i64 + 1)
    });
    assert_eq!(state_count(&db, "COMPLETED"), waiters as i64 + 1);
    drop(db);
    let drained_after = started.elapsed();

    daemon.stop(libc::SIGTERM);
    report(
        &f,
        "PROJECT_FAIRNESS_QUEUE",
        serde_json::json!({"waiters":waiters,"queued_after_ms":queued_after.as_millis(),
            "drained_after_ms":drained_after.as_millis()}),
    );
}

/// The job of the wrapper process `observer`.
fn job_of(db: &rusqlite::Connection, observer: u32) -> String {
    db.query_row(
        "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1",
        [observer],
        |row| row.get(0),
    )
    .unwrap()
}

/// The raw recorded decision and reason of one job, newest first.
fn last_decision(db: &rusqlite::Connection, job: &str) -> (String, String) {
    db.query_row(
        "SELECT decision,reason_code FROM policy_decisions WHERE job_id=?1 ORDER BY rowid DESC LIMIT 1",
        [job],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .unwrap()
}

/// Read job diagnostics through the isolated built CLI.
fn job_diagnostics(f: &Fixture, s: &Sandbox, job: &str, json: bool) -> String {
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

/// AT-TRF-08: built CLI, real daemon, offline cargo, and 101 queued waiters.
/// Each enqueue is observed; no child starts or leaves while the permit is held.
#[test]
fn at_trf_08_project_b_is_granted_after_at_most_one_project_a_job() {
    const PROJECT_A_JOBS: usize = 100;
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    s.stub("cargo");
    let (held_project, a, b) = (
        s.project("held"),
        s.project("project-a"),
        s.project("project-b"),
    );
    // Two working directories of project A: its root and a nested directory.
    let nested = a.join("crates/nested");
    std::fs::create_dir_all(&nested).unwrap();
    let (a_starts, b_starts) = (f.root.join("a-starts"), f.root.join("b-starts"));
    let mut daemon = f.spawn_isolated("");
    let started = Instant::now();

    let hold = |name: &str, project: &Path, marker: &Path| {
        let control = f.root.join(format!("{name}-control"));
        let listener = UnixListener::bind(&control).unwrap();
        let mut command = build(&f, &s, project, marker, name);
        command.env("ARC_FIXTURE_CONTROL", &control);
        (command, listener)
    };
    let held_starts = f.root.join("held-starts");
    let (mut holder, listener) = hold("holder", &held_project, &held_starts);
    let mut holder = Owned::spawn(&mut holder).unwrap();
    let db = journey::database(&f);
    let mut release_holder = accept_running(&db, &mut holder, &listener, &held_starts, "holder");

    // Observe each enqueue: 100 A builds, then one B build.
    let mut project_a = Vec::with_capacity(PROJECT_A_JOBS);
    for index in 0..PROJECT_A_JOBS {
        let name = format!("a-{index}");
        let cwd = if index % 2 == 0 { &a } else { &nested };
        let mut waiter = Owned::spawn(&mut build(&f, &s, cwd, &a_starts, &name)).unwrap();
        wait_queued(&db, &mut waiter, &a_starts, &name);
        project_a.push(waiter);
    }
    let (mut b_command, b_listener) = hold("b-0", &b, &b_starts);
    let mut project_b = Owned::spawn(&mut b_command).unwrap();
    wait_queued(&db, &mut project_b, &b_starts, "b-0");
    let queued_after = started.elapsed();
    let job_b = job_of(&db, project_b.child.id());

    let counts = || counts(&db);
    let granted_in = |project: &str| -> i64 {
        db.query_row(
            "SELECT count(*) FROM traffic_entries WHERE granted_at_ms IS NOT NULL AND project_key=\
             (SELECT project_key FROM traffic_entries WHERE job_id=?1)",
            [project],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(counts(), (PROJECT_A_JOBS as i64 + 1, 1, 1));
    assert_eq!((starts(&a_starts), starts(&b_starts)), (0, 0));
    for (index, waiter) in project_a.iter_mut().enumerate() {
        assert_queued(&db, waiter, &a_starts, &format!("a-{index}"));
    }
    // Both A directories share one opaque key; B and the holder have their own.
    let keys: Vec<(String, i64)> = db
        .prepare("SELECT project_key,count(*) FROM traffic_entries GROUP BY project_key ORDER BY count(*)")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        keys.iter().map(|(_, jobs)| *jobs).collect::<Vec<_>>(),
        [1, 1, PROJECT_A_JOBS as i64]
    );
    for (key, _) in &keys {
        assert!(
            key.len() == 64
                && key
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "{key}"
        );
    }
    let job_a: Vec<String> = project_a
        .iter()
        .map(|waiter| job_of(&db, waiter.child.id()))
        .collect();
    assert_eq!(granted_in(&job_a[0]), 0);

    // B holds the permit while we count A grants that preceded it.
    release_holder.write_all(b"x").unwrap();
    assert!(holder.wait("held build").unwrap().success());
    let mut release_b = accept_running(&db, &mut project_b, &b_listener, &b_starts, "b-0");
    let before_b = granted_in(&job_a[0]);
    assert!(
        before_b <= 1,
        "{before_b} project A builds were granted before B"
    );
    assert_eq!(counts().0 + before_b, PROJECT_A_JOBS as i64);
    let granted_after = started.elapsed();

    // Check B's fairness reason in SQLite, CLI JSON and text.
    assert_eq!(
        last_decision(&db, &job_b),
        ("admit".to_owned(), "project_fair_turn".to_owned())
    );
    let json: Value = serde_json::from_str(&job_diagnostics(&f, &s, &job_b, true)).unwrap();
    assert_eq!(json["last_recorded_reason"], "project_fair_turn");
    assert_eq!(json["admission"]["last_decision"]["kind"], "admit");
    assert_eq!(json["admission"]["last_decision"]["reason"], "admitted");
    let text = job_diagnostics(&f, &s, &job_b, false);
    assert!(
        text.contains("Last recorded decision: project_fair_turn\n"),
        "{text}"
    );
    // The earliest passed-over A build reports the class window.
    let passed = &job_a[before_b as usize];
    eventually("the passed-over build records the class window", || {
        last_decision(&db, passed) == ("wait".to_owned(), "class_window".to_owned())
    });
    let waiting: Value = serde_json::from_str(&job_diagnostics(&f, &s, passed, true)).unwrap();
    assert_eq!(waiting["admission"]["wait_reason"], "class_window");
    assert!(waiting.get("last_recorded_reason").is_none(), "{waiting}");
    let waiting_text = job_diagnostics(&f, &s, passed, false);
    assert!(
        waiting_text.contains("Last recorded decision: class_window\n"),
        "{waiting_text}"
    );
    // No output names a working directory, and no stored key is a path.
    for output in [
        &text,
        &waiting_text,
        &json.to_string(),
        &waiting.to_string(),
    ] {
        assert!(!output.contains("project-"), "{output}");
    }

    // While B holds the permit, cancel all but two queued A builds.
    let keep = before_b as usize + 2;
    for (index, waiter) in project_a.iter_mut().enumerate().skip(keep) {
        assert_eq!(
            unsafe { libc::kill(waiter.child.id() as i32, libc::SIGTERM) },
            0
        );
        let status = waiter.wait(&format!("cancelled a-{index}")).unwrap();
        assert!(!status.success(), "a-{index}: {status:?}");
        assert_eq!(
            queue_state(&db, waiter.child.id()).as_deref(),
            Some("CANCELLED"),
            "a-{index}"
        );
    }
    let cancelled = (PROJECT_A_JOBS - keep) as i64;
    assert_eq!(starts(&a_starts), before_b as usize);
    assert_eq!(counts(), (2, 1, before_b + 2));
    let cancelled_after = started.elapsed();

    // The remaining A builds run in queue order after B is released.
    release_b.write_all(b"x").unwrap();
    assert!(project_b.wait("project B build").unwrap().success());
    for (index, waiter) in project_a.iter_mut().enumerate().take(keep) {
        let status = waiter.wait(&format!("a-{index}")).unwrap();
        assert!(status.success(), "a-{index}: {status:?}");
    }
    let ran: Vec<u32> = project_a[..keep].iter().map(|w| w.child.id()).collect();
    assert_eq!(
        granted_order(&db, &ran),
        ran,
        "project A stays first in, first out"
    );
    assert_eq!(starts(&a_starts), keep, "each project A build ran once");
    assert_eq!(starts(&b_starts), 1);
    assert_eq!(starts(&f.root.join("held-starts")), 1);
    let finished = keep as i64 + 2;
    eventually("all permits released", || counts() == (0, 0, finished));
    assert_eq!(
        (state_count(&db, "COMPLETED"), state_count(&db, "CANCELLED")),
        (finished, cancelled)
    );
    assert_eq!(
        last_decision(&db, &job_b),
        ("admit".to_owned(), "project_fair_turn".to_owned())
    );
    drop(db);
    let drained_after = started.elapsed();

    daemon.stop(libc::SIGTERM);
    report(
        &f,
        "PROJECT_FAIRNESS_AT_TRF_08",
        serde_json::json!({"project_a_jobs":PROJECT_A_JOBS,"project_a_granted_before_b":before_b,
            "queued_after_ms":queued_after.as_millis(),"b_granted_after_ms":granted_after.as_millis(),
            "cancelled_after_ms":cancelled_after.as_millis(),"drained_after_ms":drained_after.as_millis()}),
    );
}
