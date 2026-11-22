//! Agent Test journey through the installed hook (AT-TRF-05, AT-TRF-06).
//!
//! The installed hook, fed a Claude Code PreToolUse payload derived from the
//! versioned vendor contract, stands for the vendor; the command it returns
//! is the one executed. No live vendor session is run or claimed.
#[path = "support/journey.rs"]
mod journey;
#[path = "agent_test_journey/ledger.rs"]
mod ledger;
#[path = "agent_test_journey/nested.rs"]
mod nested;
#[allow(dead_code)] // The shared fixture also serves unrelated workloads.
#[path = "support/runtime.rs"]
mod runtime;
#[path = "agent_test_journey/stage.rs"]
mod stage;
mod support;

use journey::{output_with_input, Sandbox};
use runtime::{child_watchdog, Fixture};
use serde_json::{json, Value};
use stage::{Run, Tool};
use std::{fs, os::unix::ffi::OsStrExt, path::Path};

/// The recorded Claude Code 2.1.327 PreToolUse event, re-addressed to the
/// fixture project and carrying `command`.
fn pre_tool_use(project: &Path, command: &str) -> Value {
    let mut event: Value = serde_json::from_str(include_str!(
        "../../arc-cli/tests/fixtures/agent_hooks/claude-recorded-2.1.327-cargo-build.json"
    ))
    .unwrap();
    event["cwd"] = json!(project);
    event["tool_input"]["command"] = json!(command);
    event
}

/// Install the Claude hook into the sandbox HOME and return its command.
fn install_hook(f: &Fixture, s: &Sandbox, project: &Path) -> String {
    let install = child_watchdog::output(
        s.arcctl(f, project)
            .args(["hooks", "install", "claude", "--scope", "user"]),
        "install Claude hook",
    )
    .unwrap();
    assert!(install.status.success(), "{install:?}");
    let settings: Value =
        serde_json::from_slice(&fs::read(s.home.join(".claude/settings.json")).unwrap()).unwrap();
    let entry = &settings["hooks"]["PreToolUse"][0];
    assert_eq!(entry["matcher"], "Bash", "{settings}");
    entry["hooks"][0]["command"].as_str().unwrap().to_owned()
}

/// Pass one event through the installed hook and return the command the
/// vendor would execute, checking that only the command was replaced and that
/// no approval decision was emitted.
fn returned_command(
    f: &Fixture,
    s: &Sandbox,
    project: &Path,
    installed: &str,
    event: &Value,
) -> String {
    let response = output_with_input(
        &mut s.shell(f, project, installed),
        &serde_json::to_vec(event).unwrap(),
        "installed hook",
    );
    assert!(response.status.success(), "{response:?}");
    assert!(response.stderr.is_empty(), "{response:?}");
    let response: Value = serde_json::from_slice(&response.stdout).unwrap();
    let hook = &response["hookSpecificOutput"];
    assert_eq!(hook["hookEventName"], "PreToolUse", "{response}");
    assert!(hook.get("permissionDecision").is_none(), "{response}");
    assert!(hook.get("permissionDecisionReason").is_none(), "{response}");
    let mut restored = hook["updatedInput"].clone();
    let command = restored["command"].as_str().unwrap().to_owned();
    restored["command"] = event["tool_input"]["command"].clone();
    assert_eq!(restored, event["tool_input"], "{response}");
    command
}

#[test]
fn seam_installed_hook_rewrites_cargo_build_to_the_managed_route() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    s.stub("cargo");
    let marker = f.root.join("cargo-starts");
    let mut daemon = f.spawn_isolated("");

    let installed = install_hook(&f, &s, &project);
    let event = pre_tool_use(&project, "cargo build");
    let command = returned_command(&f, &s, &project, &installed, &event);
    stage::assert_managed(&command, "cargo build");
    assert_eq!(
        journey::starts(&marker),
        0,
        "the hook itself ran the command"
    );

    // The vendor's shell tool executes the returned command, not the original.
    let mut shell = s.shell(&f, &project, &command);
    shell
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", &marker)
        .env("PRIVATE_ENV", "offline-fixture-value");
    let output = output_with_input(&mut shell, b"input\n", "returned hook command");
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    assert_eq!(output.stderr, b"fixture-stderr\n", "{output:?}");
    let child: Value =
        serde_json::from_slice(output.stdout.strip_prefix(b"input\n").unwrap()).unwrap();
    assert_eq!(child["args"], json!([b"build".to_vec()]));
    assert_eq!(
        child["cwd"],
        json!(fs::canonicalize(&project).unwrap().as_os_str().as_bytes())
    );
    assert_eq!(child["environment"], json!(b"offline-fixture-value"));
    assert_eq!(
        journey::starts(&marker),
        1,
        "offline cargo must run exactly once"
    );

    // The daemon recorded one managed Build and released its one permit.
    let db = journey::database(&f);
    let recorded: Vec<(String, String, String, Option<i64>)> = db
        .prepare(
            "SELECT j.job_class,t.state,p.state,j.exit_code FROM traffic_entries t \
             JOIN jobs j ON j.id=t.job_id JOIN permits p ON p.job_id=t.job_id",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        recorded,
        [(
            "BUILD".into(),
            "COMPLETED".into(),
            "RELEASED".into(),
            Some(17)
        )]
    );
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}

/// Run `command` as the vendor's shell tool does and return what the offline
/// cargo was started with.
fn vendor_runs(
    f: &Fixture,
    s: &Sandbox,
    project: &Path,
    cargo: &Tool,
    name: &str,
    command: &str,
) -> (stage::Finished, stage::Observed) {
    let mut shell = s.shell(f, project, command);
    cargo.arm(&mut shell);
    let mut run = Run::start(&mut shell, &f.root, name);
    run.input(stage::INPUT);
    let finished = run.finish(name);
    let start = cargo.take();
    stage::assert_environment(&start, &shell);
    (finished, start)
}

#[test]
fn at_trf_06_installed_hook_cargo_test_fails_open_exactly_once() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    let cargo = Tool::cargo(&f, &s);
    // A daemon that ran and is gone: nothing is reachable, and what it left
    // behind is the record that nothing below may change.
    let mut daemon = f.spawn_isolated("");
    daemon.stop(libc::SIGTERM);
    f.report();
    let record = stage::daemon_record(&f);
    assert!(record.contains_key("arc.sqlite3") && !f.socket.exists());
    // A process of the same user in the same project, which nothing may signal.
    let mut bystander = s.shell(&f, &project, "IFS= read -r line");
    let mut bystander = Run::start(&mut bystander, &f.root, "bystander");

    let installed = install_hook(&f, &s, &project);
    let event = pre_tool_use(&project, "cargo test");
    let command = returned_command(&f, &s, &project, &installed, &event);
    stage::assert_managed(&command, "cargo test");
    assert_eq!(cargo.starts(), 0, "the hook itself ran the command");

    // The literal as the vendor's shell runs it without ARC.
    let (direct, direct_start) = vendor_runs(&f, &s, &project, &cargo, "direct", "cargo test");
    assert_eq!(cargo.starts(), 1);
    assert_eq!(direct.status.code(), Some(stage::TOOL_EXIT));
    assert_eq!(direct.stdout, stage::TOOL_STDOUT);
    assert_eq!(direct.stderr, stage::TOOL_STDERR);
    assert_eq!(direct_start.args, [b"test".to_vec()]);
    assert_eq!(
        direct_start.cwd,
        fs::canonicalize(&project).unwrap().as_os_str().as_bytes()
    );
    assert_eq!(direct_start.stdin, stage::INPUT);

    // The returned command. The tool reads its standard input to the end
    // before it writes, so holding the input open orders the two writers.
    let files = stage::names(&f.root);
    let mut shell = s.shell(&f, &project, &command);
    cargo.arm(&mut shell);
    let mut run = Run::start(&mut shell, &f.root, "managed");
    run.until("the internal-error record", || {
        fs::read(f.root.join("managed.stderr")).unwrap() == stage::TRACKING_UNAVAILABLE
    });
    run.input(stage::INPUT);
    let managed = run.finish("returned hook command");
    let start = cargo.take();
    assert_eq!(cargo.starts(), 2, "offline cargo must run exactly once");
    assert_eq!(managed.status.code(), Some(stage::TOOL_EXIT));
    assert_eq!(managed.stdout, direct.stdout);
    // Exactly one internal-error record, where contract D8 names it.
    assert_eq!(
        managed.stderr,
        [stage::TRACKING_UNAVAILABLE, &direct.stderr[..]].concat()
    );
    stage::assert_environment(&start, &shell);
    stage::assert_same_start(&start, &direct_start);

    // No Reaper or GC action was started, recorded or signalled: no daemon
    // appeared, its record is byte-identical, ARC left nothing in its runtime
    // directory, and the bystander lives until it ends by itself.
    assert!(!f.socket.exists());
    assert_eq!(stage::daemon_record(&f), record);
    let mut added = stage::names(&f.root);
    added.retain(|name| !files.contains(name));
    assert_eq!(added, ["managed.stderr", "managed.stdout"]);
    assert!(bystander.alive());

    // A failure of the hook itself blocks nothing: no decision is emitted
    // and the vendor runs the literal unchanged, outside ARC.
    let whole = serde_json::to_vec(&event).unwrap();
    let mut oversized = event.clone();
    oversized["padding"] = json!("x".repeat(1024 * 1024));
    let copy = f.root.join("arcctl-copy");
    fs::copy(journey::binary("arcctl"), &copy).unwrap();
    let removed = stage::install_workspace_hook(&f, &s, &project, &copy);
    let control = returned_command(&f, &s, &project, &removed, &event);
    assert!(control.starts_with(&format!("'{}' hook-exec", copy.display())));
    fs::remove_file(&copy).unwrap();
    for (failure, hook, payload, exit) in [
        (
            "truncated event",
            &installed,
            whole[..whole.len() / 2].to_vec(),
            0,
        ),
        (
            "oversized event",
            &installed,
            serde_json::to_vec(&oversized).unwrap(),
            0,
        ),
        ("missing hook executable", &removed, whole.clone(), 127),
    ] {
        let reply = output_with_input(&mut s.shell(&f, &project, hook), &payload, failure);
        assert_eq!(reply.status.code(), Some(exit), "{failure}: {reply:?}");
        assert_eq!(stage::vendor_command(&reply.stdout, &event), "cargo test");
        let before = cargo.starts();
        let (passed, passed_start) = vendor_runs(&f, &s, &project, &cargo, failure, "cargo test");
        assert_eq!(cargo.starts(), before + 1, "{failure}");
        assert_eq!(passed.status.code(), Some(stage::TOOL_EXIT), "{failure}");
        assert_eq!(passed.stdout, direct.stdout, "{failure}");
        assert_eq!(passed.stderr, direct.stderr, "{failure}");
        assert_eq!(passed_start, direct_start, "{failure}");
    }
    assert_eq!(stage::daemon_record(&f), record);
    assert!(bystander.alive());
    bystander.input(b"done\n");
    assert!(bystander.finish("bystander").status.success());
}

/// The daemon's own diagnostics for `job`, read through the built CLI.
fn diagnostics(f: &Fixture, s: &Sandbox, project: &Path, job: &str) -> Value {
    let output = child_watchdog::output(
        s.arcctl(f, project)
            .args(["job", "diagnostics", job, "--json"]),
        "job diagnostics",
    )
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn at_trf_05_nested_hook_test_shares_the_root_test_permit() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let project = s.project("project");
    let cargo = Tool::cargo(&f, &s);
    let tree = nested::Tree::install(&f, &s, &project);
    let mut daemon = f.spawn_isolated("");
    let db = journey::database(&f);
    db.busy_timeout(std::time::Duration::from_secs(60)).unwrap();
    ledger::watch(&db);

    let installed = install_hook(&f, &s, &project);
    let root_event = pre_tool_use(&project, "bun run test");
    // The nested command is another tool call, so the vendor gives it its own
    // id: two commands of one id would be one redelivered event (AT-TRF-09).
    let mut nested_event = pre_tool_use(&project, "cargo test");
    nested_event["tool_use_id"] = json!("toolu_01NestedAgentCommandFixture");
    let root = returned_command(&f, &s, &project, &installed, &root_event);
    stage::assert_managed(&root, "bun run test");
    assert_eq!((tree.starts(), cargo.starts()), ((vec![], vec![]), 0));

    // The same tree as the vendor runs it with no hook installed.
    let direct = tree.run(
        &f,
        &s,
        &project,
        &cargo,
        "direct",
        "bun run test",
        ":",
        &nested_event,
        |_| (),
        |_| (),
    );
    assert_eq!(direct.nested, "cargo test");
    assert_eq!(direct.finished.status.code(), Some(stage::TOOL_EXIT));
    assert_eq!(direct.finished.stdout, stage::TOOL_STDOUT);
    assert_eq!(direct.finished.stderr, stage::TOOL_STDERR);
    assert_eq!(cargo.starts(), 1);
    assert_eq!(ledger::entries(&db), []);

    let test_root = |entry: &ledger::Entry, state: &str, permit: &str| {
        assert_eq!(
            (entry.class.as_str(), entry.reason.as_str(), &entry.root),
            ("TEST", "BUN_SCRIPT_TEST", &None)
        );
        (entry.state == state && entry.permit.as_deref() == Some(permit)).then_some(())
    };
    let managed = tree.run(
        &f,
        &s,
        &project,
        &cargo,
        "managed",
        &root,
        &installed,
        &nested_event,
        |run| {
            // The root holds the one TEST permit; nothing nested has started.
            run.until("the root's running record", || {
                let entries = ledger::entries(&db);
                entries.len() == 1 && test_root(&entries[0], "RUNNING", "ACTIVE").is_some()
            });
            assert_eq!(ledger::permits(&db), (1, 1));
            assert_eq!(cargo.starts(), 1);
        },
        |run| {
            // The nested command ran once and has exited; the root is alive.
            assert_eq!(cargo.starts(), 2);
            run.until("the nested completion record", || {
                ledger::entries(&db)
                    .get(1)
                    .is_some_and(|e| e.exit.is_some())
            });
            let entries = ledger::entries(&db);
            let [outer, inner] = entries.as_slice() else {
                panic!("{entries:?}")
            };
            test_root(outer, "RUNNING", "ACTIVE").expect("the root's permit is held");
            assert_eq!(
                *inner,
                ledger::Entry {
                    job: inner.job.clone(),
                    class: "TEST".into(),
                    reason: "CARGO_TEST".into(),
                    state: "COMPLETED".into(),
                    root: Some(outer.job.clone()),
                    exit: Some(stage::TOOL_EXIT.into()),
                    permit: None,
                }
            );
            assert_eq!(ledger::permits(&db), (1, 1));
            // The daemon reused the root's reservation and never queued it.
            let admission = diagnostics(&f, &s, &project, &inner.job)["admission"].take();
            assert_eq!(admission["last_decision"]["kind"], "reuse");
            assert_eq!(admission["last_decision"]["reason"], "existing_reservation");
            assert_eq!(admission["root_job_id"], json!(outer.job));
            assert_eq!(admission["wait_reason"], Value::Null);
        },
    );

    // The nested command is the one the installed hook returned, unchanged in
    // arguments, working directory, environment, streams and exit status.
    stage::assert_managed(&managed.nested, "cargo test");
    assert_eq!(
        managed.finished.status.code(),
        direct.finished.status.code()
    );
    assert_eq!(managed.finished.stdout, direct.finished.stdout);
    // Neither wrapper wrote a QUEUED or any other notice.
    assert_eq!(managed.finished.stderr, direct.finished.stderr);
    stage::assert_same_start(&managed.cargo, &direct.cargo);
    assert_eq!(managed.cargo.args, [b"test".to_vec()]);
    assert_eq!(
        managed.cargo.cwd,
        fs::canonicalize(&project).unwrap().as_os_str().as_bytes()
    );
    assert_eq!(managed.cargo.stdin, stage::INPUT);
    // Ancestry, not an environment value, attributed the nested run.
    assert!(!managed
        .cargo
        .environment
        .contains_key("ARC_TRAFFIC_LINEAGE"));
    let twice = |argv: &str| vec![argv.to_owned(); 2];
    assert_eq!(tree.starts(), (twice("run test"), twice("")));
    assert_eq!(cargo.starts(), 2);

    // The one permit was the root's and was released after the root exited.
    journey::eventually("the root's release", || ledger::permits(&db) == (1, 0));
    let entries = ledger::entries(&db);
    test_root(&entries[0], "COMPLETED", "RELEASED").expect("the root's record");
    assert_eq!(entries[0].exit, Some(stage::TOOL_EXIT.into()));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].permit, None);
    let held = ledger::held(&db);
    assert_eq!((held.iter().max(), held.last()), (Some(&1), Some(&0)));
    ledger::unwatch(&db);
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}
