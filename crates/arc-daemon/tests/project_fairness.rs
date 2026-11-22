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
    io::Write,
    os::unix::net::UnixListener,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Instant,
};

fn diagnostics(marker: &Path, name: &str) -> PathBuf {
    marker.with_extension(format!("{name}.stderr"))
}

/// `arcctl exec -- cargo build` in `project`, with the offline cargo stub
/// recording each of its starts in `marker`. The wrapper's own diagnostics go
/// to a file inside the fixture directory.
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

/// While a separate built CLI holds the one Build permit, queue `waiters`
/// builds of one project, each enqueue confirmed from daemon state. None may
/// start or leave the queue. After the release every build runs exactly once,
/// in queue order, and all permits are released; the daemon rejected nothing.
fn queued_builds_of_one_project_drain_in_queue_order(waiters: usize) {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    s.stub("cargo");
    let (held_project, project) = (s.project("held"), s.project("project-a"));
    let marker = f.root.join("waiter-starts");
    let mut daemon = f.spawn_isolated("");
    let started = Instant::now();

    // The holder's offline cargo child reaches the barrier only after
    // admission and native spawn.
    let listener = UnixListener::bind(f.root.join("held-build")).unwrap();
    let mut holder = build(&f, &s, &held_project, &f.root.join("held-starts"), "holder");
    holder.env("ARC_FIXTURE_CONTROL", f.root.join("held-build"));
    let mut holder = Owned::spawn(&mut holder).unwrap();
    let (mut release, _) = native_exec::accept(&listener);
    let db = journey::database(&f);
    assert_eq!(
        queue_state(&db, holder.child.id()).as_deref(),
        Some("RUNNING")
    );

    let mut queued = Vec::with_capacity(waiters);
    for index in 0..waiters {
        let name = format!("waiter-{index}");
        let mut waiter = Owned::spawn(&mut build(&f, &s, &project, &marker, &name)).unwrap();
        let pid = waiter.child.id();
        eventually(&format!("{name} queued"), || {
            if let Some(status) = waiter.child.try_wait().unwrap() {
                panic!(
                    "{name} left before it was queued: {status:?}, queue state {:?}, {} child starts, stderr {:?}",
                    queue_state(&db, pid),
                    starts(&marker),
                    std::fs::read_to_string(diagnostics(&marker, &name))
                );
            }
            queue_state(&db, pid).as_deref() == Some("QUEUED")
        });
        queued.push(waiter);
    }
    let queued_after = started.elapsed();
    let counts = || -> (i64, i64, i64) {
        db.query_row(
            "SELECT (SELECT count(*) FROM traffic_entries WHERE state='QUEUED'), \
             (SELECT count(*) FROM permits WHERE state='ACTIVE'), \
             (SELECT count(*) FROM permits)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
    };
    assert_eq!(counts(), (waiters as i64, 1, 1));
    assert_eq!(starts(&marker), 0, "a queued build started its child");
    for (index, waiter) in queued.iter_mut().enumerate() {
        assert!(
            waiter.child.try_wait().unwrap().is_none(),
            "waiter {index} left the queue: {:?}",
            std::fs::read_to_string(diagnostics(&marker, &format!("waiter-{index}")))
        );
        assert_eq!(
            queue_state(&db, waiter.child.id()).as_deref(),
            Some("QUEUED")
        );
    }

    release.write_all(b"x").unwrap();
    assert!(holder.wait("held build").unwrap().success());
    for (index, waiter) in queued.iter_mut().enumerate() {
        let status = waiter.wait(&format!("waiter {index}")).unwrap();
        assert!(status.success(), "waiter {index}: {status:?}");
    }
    // With a window of one the children start strictly one after another, so
    // the start order is the grant order: it must be the queue order.
    let order: Vec<u32> = std::fs::read_to_string(&marker)
        .unwrap()
        .lines()
        .map(|child| {
            db.query_row(
                "SELECT observer_pid FROM job_lifecycle WHERE child_pid=?1",
                [child.parse::<u32>().unwrap()],
                |r| r.get(0),
            )
            .unwrap()
        })
        .collect();
    let expected: Vec<u32> = queued.iter().map(|waiter| waiter.child.id()).collect();
    assert_eq!(
        order, expected,
        "each build runs exactly once, in queue order"
    );
    eventually("all permits released", || {
        counts() == (0, 0, waiters as i64 + 1)
    });
    let completed: i64 = db
        .query_row(
            "SELECT count(*) FROM traffic_entries WHERE state='COMPLETED'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(completed, waiters as i64 + 1);
    drop(db);
    let drained_after = started.elapsed();

    daemon.stop(libc::SIGTERM);
    let report = f.report();
    let metrics = &report["metrics"];
    // Wall time and high-water marks are printed for the record, never asserted.
    println!(
        "PROJECT_FAIRNESS_QUEUE {}",
        serde_json::json!({"waiters":waiters,"queued_after_ms":queued_after.as_millis(),
            "drained_after_ms":drained_after.as_millis(),
            "connections_high_water":metrics["connections"]["high_water"],
            "requests_high_water":metrics["requests"]["high_water"],
            "db_queued_high_water":metrics["db_queued"]["high_water"],
            "frames_completed":metrics["frames_completed"]})
    );
    assert_eq!(report["profiles"]["ipc"], "default");
    assert_eq!(metrics["connection_rejected"], 0, "{report}");
    assert_eq!(metrics["request_rejected"], 0, "{report}");
    assert_eq!(metrics["db_rejected"], 0, "{report}");
}

#[test]
fn seam_two_queued_builds_keep_fifo_order_through_built_arcctl() {
    queued_builds_of_one_project_drain_in_queue_order(2);
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

/// `arcctl job diagnostics <job>` through the built CLI, isolated like every
/// other process of this target.
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

/// AT-TRF-08 on the path a user's command takes: built `arcctl exec`, a real
/// daemon at its default limits, an offline cargo stub and a Build window of
/// one. It also holds the contract's 101-waiter probe: every enqueue is
/// confirmed from daemon state, nothing starts or leaves the queue while the
/// permit is held, and the daemon rejects no connection, request or ticket.
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
    let (mut holder, listener) = hold("holder", &held_project, &f.root.join("held-starts"));
    let mut holder = Owned::spawn(&mut holder).unwrap();
    let (mut release_holder, _) = native_exec::accept(&listener);
    let db = journey::database(&f);
    assert_eq!(
        queue_state(&db, holder.child.id()).as_deref(),
        Some("RUNNING")
    );

    // Project A queues 100 builds, then project B queues one. Each enqueue is
    // confirmed from daemon state before the next wrapper starts.
    let queue = |waiter: &mut Owned, name: &str, marker: &Path| {
        let pid = waiter.child.id();
        eventually(&format!("{name} queued"), || {
            if let Some(status) = waiter.child.try_wait().unwrap() {
                panic!(
                    "{name} left before it was queued: {status:?}, queue state {:?}, stderr {:?}",
                    queue_state(&db, pid),
                    std::fs::read_to_string(diagnostics(marker, name))
                );
            }
            queue_state(&db, pid).as_deref() == Some("QUEUED")
        });
    };
    let mut project_a = Vec::with_capacity(PROJECT_A_JOBS);
    for index in 0..PROJECT_A_JOBS {
        let name = format!("a-{index}");
        let cwd = if index % 2 == 0 { &a } else { &nested };
        let mut waiter = Owned::spawn(&mut build(&f, &s, cwd, &a_starts, &name)).unwrap();
        queue(&mut waiter, &name, &a_starts);
        project_a.push(waiter);
    }
    let (mut b_command, b_listener) = hold("b-0", &b, &b_starts);
    let mut project_b = Owned::spawn(&mut b_command).unwrap();
    queue(&mut project_b, "b-0", &b_starts);
    let queued_after = started.elapsed();
    let job_b = job_of(&db, project_b.child.id());

    let counts = || -> (i64, i64, i64) {
        db.query_row(
            "SELECT (SELECT count(*) FROM traffic_entries WHERE state='QUEUED'), \
             (SELECT count(*) FROM permits WHERE state='ACTIVE'), \
             (SELECT count(*) FROM permits)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
    };
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
        assert!(
            waiter.child.try_wait().unwrap().is_none(),
            "a-{index} left the queue: {:?}",
            std::fs::read_to_string(diagnostics(&a_starts, &format!("a-{index}")))
        );
        assert_eq!(
            queue_state(&db, waiter.child.id()).as_deref(),
            Some("QUEUED")
        );
    }
    // Each job carries its project as an opaque digest: one key for both
    // working directories of project A, another for B, a third for the holder.
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

    // Release the held permit. Project B's build reaches its barrier holding
    // the one Build permit, so the count below is read while nothing else can
    // be granted: at most one build of project A went first.
    release_holder.write_all(b"x").unwrap();
    assert!(holder.wait("held build").unwrap().success());
    let (mut release_b, _) = native_exec::accept(&b_listener);
    assert_eq!(
        queue_state(&db, project_b.child.id()).as_deref(),
        Some("RUNNING")
    );
    let before_b = granted_in(&job_a[0]);
    assert!(
        before_b <= 1,
        "{before_b} project A builds were granted before B"
    );
    assert_eq!(counts().0 + before_b, PROJECT_A_JOBS as i64);
    let granted_after = started.elapsed();

    // B's record carries the fairness reason: raw in SQLite, in the CLI's
    // JSON beside the unchanged v1 object, and in the rendered text.
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
    // The earliest project A build still queued was passed over for a later
    // entry: its recorded cause is the class window, never an earlier job.
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

    // While B holds the permit, cancel all but two of the queued project A
    // builds, one at a time: each ends cancelled and never starts its child.
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

    // Release B. The remaining project A builds run in queue order, so every
    // job ran exactly once or was cancelled with zero starts.
    release_b.write_all(b"x").unwrap();
    assert!(project_b.wait("project B build").unwrap().success());
    for (index, waiter) in project_a.iter_mut().enumerate().take(keep) {
        let status = waiter.wait(&format!("a-{index}")).unwrap();
        assert!(status.success(), "a-{index}: {status:?}");
    }
    let order: Vec<u32> = std::fs::read_to_string(&a_starts)
        .unwrap()
        .lines()
        .map(|child| {
            db.query_row(
                "SELECT observer_pid FROM job_lifecycle WHERE child_pid=?1",
                [child.parse::<u32>().unwrap()],
                |r| r.get(0),
            )
            .unwrap()
        })
        .collect();
    let ran: Vec<u32> = project_a[..keep].iter().map(|w| w.child.id()).collect();
    assert_eq!(order, ran, "project A stays first in, first out");
    assert_eq!(starts(&b_starts), 1);
    assert_eq!(starts(&f.root.join("held-starts")), 1);
    let finished = keep as i64 + 2;
    eventually("all permits released", || counts() == (0, 0, finished));
    let states = |state: &str| -> i64 {
        db.query_row(
            "SELECT count(*) FROM traffic_entries WHERE state=?1",
            [state],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(
        (states("COMPLETED"), states("CANCELLED")),
        (finished, cancelled)
    );
    assert_eq!(
        last_decision(&db, &job_b),
        ("admit".to_owned(), "project_fair_turn".to_owned())
    );
    drop(db);
    let drained_after = started.elapsed();

    daemon.stop(libc::SIGTERM);
    let report = f.report();
    let metrics = &report["metrics"];
    // Wall time and high-water marks are printed for the record, never asserted.
    println!(
        "PROJECT_FAIRNESS_AT_TRF_08 {}",
        serde_json::json!({"project_a_jobs":PROJECT_A_JOBS,"project_a_granted_before_b":before_b,
            "queued_after_ms":queued_after.as_millis(),"b_granted_after_ms":granted_after.as_millis(),
            "cancelled_after_ms":cancelled_after.as_millis(),"drained_after_ms":drained_after.as_millis(),
            "connections_high_water":metrics["connections"]["high_water"],
            "requests_high_water":metrics["requests"]["high_water"],
            "db_queued_high_water":metrics["db_queued"]["high_water"],
            "frames_completed":metrics["frames_completed"]})
    );
    assert_eq!(report["profiles"]["ipc"], "default");
    assert_eq!(metrics["connection_rejected"], 0, "{report}");
    assert_eq!(metrics["request_rejected"], 0, "{report}");
    assert_eq!(metrics["db_rejected"], 0, "{report}");
}
