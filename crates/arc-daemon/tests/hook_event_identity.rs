//! Vendor hook event identity through the installed hook, `hook-exec` and a
//! real daemon (AT-TRF-09, ADR-014). Tools are offline stubs; the Build
//! window is 1.
//!
//! The offline `cargo` records what it was started with and then reads its
//! standard input to the end, so an execution whose input the test still
//! holds open is running with its permit held. Every wait is on observed
//! state; the 60 s deadlines are diagnostic watchdogs.
#[path = "hook_event_identity/hook.rs"]
mod hook;
#[path = "support/journey.rs"]
mod journey;
#[allow(dead_code)] // Only the permit ledger of the agent journey is used here.
#[path = "agent_test_journey/ledger.rs"]
mod ledger;
#[path = "hook_event_identity/record.rs"]
mod record;
#[path = "hook_event_identity/redelivery.rs"]
mod redelivery;
#[allow(dead_code)] // The shared fixture also serves unrelated workloads.
#[path = "support/runtime.rs"]
mod runtime;
#[allow(dead_code)] // Only the offline tool and the vendor's shell are used here.
#[path = "agent_test_journey/stage.rs"]
mod stage;
mod support;

use hook::Vendor;
use journey::{output_with_input, starts, Sandbox};
use redelivery::World;
use runtime::Fixture;
use serde_json::{json, Value};

/// Job class, queue state, permit state and exit code of every managed job.
fn recorded(db: &rusqlite::Connection) -> Vec<(String, String, String, Option<i64>)> {
    db.prepare(
        "SELECT j.job_class,t.state,p.state,j.exit_code FROM traffic_entries t \
         JOIN jobs j ON j.id=t.job_id JOIN permits p ON p.job_id=t.job_id ORDER BY t.sequence",
    )
    .unwrap()
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

fn keyed_jobs(db: &rusqlite::Connection) -> i64 {
    db.query_row(
        "SELECT count(*) FROM job_lifecycle WHERE event_key IS NOT NULL",
        [],
        |r| r.get(0),
    )
    .unwrap()
}

/// An event that carries neither a session id nor a tool call id has no
/// identity: for each of the four managed literals the installed hook returns
/// exactly today's command, and executing it records an ordinary job with no
/// event key.
#[test]
fn seam_installed_hook_keeps_todays_command_for_a_payload_without_event_ids() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    s.stub("cargo");
    let marker = f.root.join("cargo-starts");
    let mut daemon = f.spawn_isolated("");
    let installed = Vendor::Claude.install(&f, &s, &project);

    let without_ids = |command: &str| {
        let mut event = hook::claude_event(&project, command);
        let fields = event.as_object_mut().unwrap();
        assert!(fields.remove("session_id").is_some());
        assert!(fields.remove("tool_use_id").is_some());
        event
    };
    let mut build = String::new();
    for literal in ["cargo build", "bun run build", "cargo test", "bun run test"] {
        let event = without_ids(literal);
        let command = Vendor::Claude.returned_command(&f, &s, &project, &installed, &event);
        let quoted = hook::quoted_arcctl(&command, &format!(" hook-exec -- {literal}"));
        assert_eq!(command, format!("{quoted} hook-exec -- {literal}"));
        if literal == "cargo build" {
            build = command;
        }
    }
    assert_eq!(starts(&marker), 0, "the hook itself ran a command");

    // The vendor's shell tool executes the returned command.
    let mut shell = s.shell(&f, &project, &build);
    shell
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", &marker);
    let output = output_with_input(&mut shell, b"input\n", "returned hook command");
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    assert_eq!(output.stderr, b"fixture-stderr\n", "{output:?}");
    assert_eq!(starts(&marker), 1, "offline cargo must run exactly once");
    let db = journey::database(&f);
    assert_eq!(
        recorded(&db),
        [(
            "BUILD".into(),
            "COMPLETED".into(),
            "RELEASED".into(),
            Some(17)
        )]
    );
    assert_eq!(keyed_jobs(&db), 0);
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}

/// `hook-exec` consumes `--arc-event <key>` placed directly after it: the
/// child receives the original argv, runs once and keeps its streams and exit
/// status. A value that is not 64 lowercase hexadecimal digits was not
/// written by the hook and starts no child.
#[test]
fn seam_hook_exec_consumes_the_event_option_and_rejects_a_malformed_key() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    s.stub("cargo");
    let marker = f.root.join("cargo-starts");
    let mut daemon = f.spawn_isolated("");
    let key = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    let mut keyed = s.arcctl(&f, &project);
    keyed
        .args(["hook-exec", "--arc-event", key, "--", "cargo", "build"])
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", &marker);
    let output = output_with_input(&mut keyed, b"input\n", "keyed hook-exec");
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    assert_eq!(output.stderr, b"fixture-stderr\n", "{output:?}");
    let child: Value =
        serde_json::from_slice(output.stdout.strip_prefix(b"input\n").unwrap()).unwrap();
    assert_eq!(child["args"], json!([b"build".to_vec()]));
    assert_eq!(starts(&marker), 1);
    let db = journey::database(&f);
    assert_eq!(
        recorded(&db),
        [(
            "BUILD".into(),
            "COMPLETED".into(),
            "RELEASED".into(),
            Some(17)
        )]
    );

    for malformed in ["", "not-a-key", &key[..63], &key.to_uppercase()] {
        let mut rejected = s.arcctl(&f, &project);
        rejected
            .args([
                "hook-exec",
                "--arc-event",
                malformed,
                "--",
                "cargo",
                "build",
            ])
            .env("ARC_FIXTURE_MODE", "echo")
            .env("ARC_FIXTURE_MARKER", &marker);
        let output = output_with_input(&mut rejected, b"", "malformed event key");
        assert_eq!(output.status.code(), Some(2), "{malformed:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{malformed:?}: {output:?}");
        assert_eq!(
            output.stderr,
            b"INVALID_REQUEST\nUse arcctl exec -- <PROGRAM> [ARG...]. No child was started.\n",
            "{malformed:?}"
        );
    }
    assert_eq!(starts(&marker), 1, "a rejected key started a child");
    assert_eq!(recorded(&db).len(), 1, "a rejected key registered a job");
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}

fn running(db: &rusqlite::Connection) -> Option<String> {
    let entries = ledger::entries(db);
    (entries.len() == 1
        && entries[0].state == "RUNNING"
        && entries[0].permit.as_deref() == Some("ACTIVE"))
    .then(|| entries[0].job.clone())
}

fn completed(job: &str) -> ledger::Entry {
    ledger::Entry {
        job: job.to_owned(),
        class: "BUILD".into(),
        reason: "CARGO_BUILD".into(),
        state: "COMPLETED".into(),
        root: None,
        exit: Some(stage::TOOL_EXIT.into()),
        permit: Some("RELEASED".into()),
    }
}

/// Jobs for which the daemon recorded any admission decision.
fn decided(db: &rusqlite::Connection) -> Vec<String> {
    db.prepare("SELECT DISTINCT job_id FROM policy_decisions WHERE job_id IS NOT NULL")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn open(w: &World) -> rusqlite::Connection {
    let db = journey::database(&w.f);
    db.busy_timeout(std::time::Duration::from_secs(60)).unwrap();
    ledger::watch(&db);
    db
}

/// The vendor delivers one recorded PreToolUse event twice. The installed
/// hook returns the same keyed command both times and both are executed. The
/// first holds the one Build permit while the redelivery runs to its end:
/// one job and one permit exist for the key, the redelivery waited for
/// nothing and printed nothing, each execution ran the tool once and
/// unchanged, and the redelivery is recorded on the existing job.
fn redelivered_event_creates_one_job_and_one_permit(vendor: Vendor) {
    let w = World::new();
    let mut daemon = w.f.spawn_isolated("");
    let db = open(&w);
    let installed = vendor.install(&w.f, &w.s, &w.project);
    let event = vendor.recorded();

    let first = vendor.returned_command(&w.f, &w.s, &w.project, &installed, &event);
    let second = vendor.returned_command(&w.f, &w.s, &w.project, &installed, &event);
    let key = hook::event_key(&first, "cargo build");
    assert_eq!(second, first, "one event names one key");
    for id in ["session_id", "tool_use_id"] {
        assert!(!first.contains(event[id].as_str().unwrap()), "{first}");
    }
    assert_eq!(w.cargo.starts(), 0, "the hook itself ran a command");

    // The literal as the vendor's shell runs it without ARC.
    let direct = w.run("direct", "cargo build");
    assert_eq!(ledger::entries(&db), []);

    // First delivery: an ordinary managed job that holds the Build window.
    let mut one = w.start("first", &first);
    let mut job = None;
    one.run.until("the first delivery's running record", || {
        job = running(&db);
        job.is_some()
    });
    let job = job.unwrap();
    let recorded_key = [(job.clone(), Some(key.clone()))];
    assert_eq!(record::keys(&db), recorded_key);
    assert_eq!(record::rows(&db), [1; 5]);
    assert_eq!(w.cargo.starts(), 2);

    // Redelivery: it runs to its end while the window is full, so it was
    // not admitted, not queued and did not wait.
    let duplicate = w.start("duplicate", &second).finish(&w);
    assert!(one.run.alive(), "the first delivery ended early");
    assert_eq!(w.cargo.starts(), 3, "the redelivery ran its tool once");
    // No QUEUED notice and no other ARC text on its streams.
    redelivery::assert_unchanged(&w, &duplicate, &direct, b"");
    assert_eq!(running(&db).as_ref(), Some(&job));
    assert_eq!(record::rows(&db), [1; 5], "one job, entry and permit");
    assert_eq!(ledger::permits(&db), (1, 1));
    assert_eq!(record::keys(&db), recorded_key);
    let only_job = std::slice::from_ref(&job);
    assert_eq!(record::duplicates(&db), only_job);
    assert_eq!(decided(&db), only_job);

    // The first delivery ends as the one job and releases the one permit.
    let first_done = one.finish(&w);
    redelivery::assert_unchanged(&w, &first_done, &direct, b"");
    assert_eq!(w.cargo.starts(), 3, "each execution ran its tool once");
    journey::eventually("the release", || ledger::permits(&db) == (1, 0));
    assert_eq!(ledger::entries(&db), [completed(&job)]);
    assert_eq!(record::rows(&db), [1; 5]);
    assert_eq!(record::keys(&db), recorded_key);
    assert_eq!(record::duplicates(&db), only_job);
    assert_eq!(decided(&db), only_job);
    let held = ledger::held(&db);
    assert_eq!((held.iter().max(), held.last()), (Some(&1), Some(&0)));
    ledger::unwatch(&db);
    drop(db);

    // Neither raw id reached SQLite or any stream, before or after shutdown.
    record::assert_no_raw_ids(&w.f, &event);
    daemon.stop(libc::SIGTERM);
    w.f.report();
    record::assert_no_raw_ids(&w.f, &event);
}

#[test]
fn at_trf_09_redelivered_claude_event_creates_one_job_and_one_permit() {
    redelivered_event_creates_one_job_and_one_permit(Vendor::Claude);
}

#[test]
fn at_trf_09_redelivered_cursor_event_creates_one_job_and_one_permit() {
    redelivered_event_creates_one_job_and_one_permit(Vendor::Cursor);
}

/// Control: another tool call of the same session is another event. It is
/// admitted as its own job, so with the window full it queues, says so, and
/// starts only after the first job's release.
#[test]
fn at_trf_09_distinct_tool_use_ids_create_two_jobs() {
    let w = World::new();
    let mut daemon = w.f.spawn_isolated("");
    let db = open(&w);
    let vendor = Vendor::Claude;
    let installed = vendor.install(&w.f, &w.s, &w.project);
    let event = vendor.recorded();
    // A derived input: the id a vendor gives its next tool call.
    let mut next = event.clone();
    next["tool_use_id"] = json!(format!("{}-next", event["tool_use_id"].as_str().unwrap()));
    let first = vendor.returned_command(&w.f, &w.s, &w.project, &installed, &event);
    let second = vendor.returned_command(&w.f, &w.s, &w.project, &installed, &next);
    let keys = [
        hook::event_key(&first, "cargo build"),
        hook::event_key(&second, "cargo build"),
    ];
    assert_ne!(keys[0], keys[1]);
    let direct = w.run("direct", "cargo build");

    let mut one = w.start("first", &first);
    one.run.until("the first event's running record", || {
        running(&db).is_some()
    });
    let mut two = w.start("second", &second);
    two.run.until("the second event's queued record", || {
        ledger::entries(&db)
            .get(1)
            .is_some_and(|entry| entry.state == "QUEUED" && entry.permit.is_none())
    });
    two.run.until("the QUEUED notice", || {
        w.stderr("second")
            .starts_with(b"QUEUED\nWaiting to start.\n")
    });
    assert_eq!(w.cargo.starts(), 2, "the queued event started its tool");
    assert_eq!(ledger::permits(&db), (1, 1));
    let jobs: Vec<String> = ledger::entries(&db).into_iter().map(|e| e.job).collect();
    assert_eq!(
        record::keys(&db),
        [
            (jobs[0].clone(), Some(keys[0].clone())),
            (jobs[1].clone(), Some(keys[1].clone()))
        ]
    );

    let first_done = one.finish(&w);
    redelivery::assert_unchanged(&w, &first_done, &direct, b"");
    two.run
        .until("the second event's start", || w.cargo.starts() == 3);
    let second_done = two.finish(&w);
    assert_eq!(second_done.finished.status, direct.finished.status);
    assert_eq!(second_done.finished.stdout, direct.finished.stdout);
    assert!(second_done.finished.stderr.ends_with(stage::TOOL_STDERR));
    stage::assert_same_start(&second_done.start, &direct.start);
    journey::eventually("both releases", || ledger::permits(&db) == (2, 0));
    assert_eq!(
        ledger::entries(&db),
        [completed(&jobs[0]), completed(&jobs[1])]
    );
    assert_eq!(record::rows(&db), [2; 5]);
    assert!(record::duplicates(&db).is_empty());
    let held = ledger::held(&db);
    assert_eq!((held.iter().max(), held.last()), (Some(&1), Some(&0)));
    ledger::unwatch(&db);
    drop(db);
    daemon.stop(libc::SIGTERM);
    w.f.report();
}

/// Control: with no daemon reachable both deliveries of the event run the
/// tool exactly once each, unchanged, with the one fail-open record.
#[test]
fn at_trf_09_redelivered_event_fails_open_without_a_daemon() {
    let w = World::new();
    // A daemon that ran and is gone; what it left may not change below.
    let mut daemon = w.f.spawn_isolated("");
    daemon.stop(libc::SIGTERM);
    w.f.report();
    let left = stage::daemon_record(&w.f);
    assert!(!w.f.socket.exists());

    let vendor = Vendor::Claude;
    let installed = vendor.install(&w.f, &w.s, &w.project);
    let event = vendor.recorded();
    let first = vendor.returned_command(&w.f, &w.s, &w.project, &installed, &event);
    let second = vendor.returned_command(&w.f, &w.s, &w.project, &installed, &event);
    hook::event_key(&first, "cargo build");
    assert_eq!(second, first);
    let direct = w.run("direct", "cargo build");

    for (index, (name, command)) in [("first", &first), ("duplicate", &second)]
        .into_iter()
        .enumerate()
    {
        // The tool writes after its input ends, which orders the two writers.
        let mut execution = w.start(name, command);
        execution.run.until("the fail-open record", || {
            w.stderr(name) == stage::TRACKING_UNAVAILABLE
        });
        let done = execution.finish(&w);
        assert_eq!(w.cargo.starts(), index + 2, "{name} ran its tool once");
        redelivery::assert_unchanged(&w, &done, &direct, stage::TRACKING_UNAVAILABLE);
    }
    assert!(!w.f.socket.exists());
    assert_eq!(stage::daemon_record(&w.f), left);
}

/// The key is sent only to a daemon that advertises the capability. A file
/// that no process locks stands for a daemon that does not: both deliveries
/// are then ordinary jobs with no recorded key, exactly as before.
#[test]
fn at_trf_09_a_daemon_that_does_not_advertise_receives_no_key() {
    let w = World::new();
    let mut daemon = w.f.spawn_isolated("");
    let db = open(&w);
    let advertisement =
        w.f.socket
            .parent()
            .unwrap()
            .join(arc_ipc::traffic::hook_event::ADVERTISEMENT);
    assert!(advertisement.is_file(), "the daemon advertised nothing");
    std::fs::remove_file(&advertisement).unwrap();
    std::fs::write(&advertisement, b"").unwrap();

    let vendor = Vendor::Claude;
    let installed = vendor.install(&w.f, &w.s, &w.project);
    let event = vendor.recorded();
    let command = vendor.returned_command(&w.f, &w.s, &w.project, &installed, &event);
    hook::event_key(&command, "cargo build");
    let direct = w.run("direct", "cargo build");
    for name in ["first", "second"] {
        let done = w.run(name, &command);
        redelivery::assert_unchanged(&w, &done, &direct, b"");
    }
    assert_eq!(w.cargo.starts(), 3);
    journey::eventually("both releases", || ledger::permits(&db) == (2, 0));
    let entries = ledger::entries(&db);
    assert_eq!(
        entries,
        [completed(&entries[0].job), completed(&entries[1].job)]
    );
    assert_eq!(
        record::keys(&db),
        [
            (entries[0].job.clone(), None),
            (entries[1].job.clone(), None)
        ]
    );
    assert!(record::duplicates(&db).is_empty());
    ledger::unwatch(&db);
    drop(db);
    daemon.stop(libc::SIGTERM);
    w.f.report();
}
