//! Local container builds (AT-TRF-03, DET-011 HOST items).
//! Lane C owns this module; `closing_locality.rs` only registers it.
//!
//! Every local-proof test is macOS-only: the socket inventory that proves a
//! local engine is injected through `DYLD_INSERT_LIBRARIES`, and no other
//! platform has a listener inventory yet.
use super::*;

#[path = "local_container/fixture.rs"]
mod fixture;
#[cfg(target_os = "macos")]
#[path = "local_container/readers.rs"]
mod readers;

use arc_ipc::compute_location::OPERATION_JOB_STATUS as NEGOTIATED_STATUS;
use fixture::*;

/// One recorded classification as each reader receives it.
pub(super) fn recorded(f: &Fixture, operation: &str, id: &JobId) -> Value {
    job(f, operation, id)["classification"].clone()
}

// Contract seam: a remote Docker context stays REMOTE_EXEC without local
// admission, and neither the projection nor the negotiated reader changes it.
#[test]
fn container_seam_remote_context_is_unchanged_for_every_reader() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    super::super::endpoints::install(&s);
    prod_context(&s, "tcp://192.0.2.30:2376");
    let mut daemon = f.spawn_isolated("test");
    let cases: [(&str, &[&str]); 2] = [
        (
            "remote context",
            &["docker", "--context", "prod", "build", "."],
        ),
        ("remote builder", &["docker", "buildx", "build", "."]),
    ];
    for (name, argv) in cases {
        let before = admission_counts(&f);
        let inspection = s.classify(&f, argv, &[]);
        let (job, id) = s.exec(&f, argv, &[]);
        assert!(!assert_routes(name, &inspection, &job, "REMOTE_EXEC"));
        assert_recorded_fixture_child(&s, &job);
        assert_eq!(admission_counts(&f), before, "{name} added local admission");

        let remote = json!({"classification":"REMOTE_EXEC","locality":"REMOTE",
            "reason":"REMOTE_ENDPOINT_EXEC","uncertainty":"NONE"});
        // Readers that predate the vocabulary get the projection, which leaves
        // a remote record as it is; the negotiated reader names the same record.
        assert_eq!(
            recorded(&f, OPERATION_JOB_STATUS_LOCALITY, &id),
            remote,
            "{name}"
        );
        assert_eq!(recorded(&f, NEGOTIATED_STATUS, &id), remote, "{name}");
        // Protocol-1 keeps its own vocabulary.
        assert_eq!(
            recorded(&f, arc_ipc::jobs::OPERATION_JOB_STATUS, &id),
            json!({"classification":"UNKNOWN","locality":"UNKNOWN",
                "reason":"UNRESOLVED_COMMAND","uncertainty":"OBSERVE"}),
            "{name}"
        );
    }
    assert_eq!(s.calls(), 2);
    daemon.stop(libc::SIGTERM);
    f.report();
}

/// One build with a proven local context and builder on one production route:
/// recorded BUILD in a Local Container, reserved against the global budget, and
/// not started while another Build permit is held.
#[cfg(target_os = "macos")]
fn local_container_build(route: Route, argv: &[&str]) {
    use std::os::unix::net::UnixListener;
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let engine = LocalEngine::proven(&f, &s);
    let env = engine.env();
    let label = format!("{route:?} {argv:?}");
    symlink(cli::binary("arc-exec-fixture"), s.bin.join("cargo")).unwrap();
    let mut daemon = f.spawn_isolated("test");

    if route == Route::HookExec {
        // The installed hook returns exactly the argv this test then runs.
        let arcctl = cli::binary("arcctl");
        assert_eq!(
            installed_hook_rewrite(&s, &f, &argv.join(" ")),
            Some(format!(
                "'{}' hook-exec -- {}",
                arcctl.display(),
                argv.join(" ")
            )),
            "{label}"
        );
    }
    let inspection = s.classify(&f, argv, &env);
    assert_eq!(inspection, container_record("LOCAL_CONTAINER"), "{label}");

    // A separate real CLI owns the one Build permit. Its offline cargo child
    // reaches the barrier only after admission and native spawn.
    let listener = UnixListener::bind(f.root.join("held-build")).unwrap();
    let mut held = s.command(&f, &[]);
    held.env("ARC_FIXTURE_CONTROL", f.root.join("held-build"))
        .env("ARC_FIXTURE_MARKER", f.root.join("held-calls"))
        .args(["exec", "--", "cargo", "build"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut holder =
        crate::runtime::OwnedChild::spawn_with_descendants(held, f.root.join("held-owners"))
            .unwrap();
    let (mut release, held_child) = cli::accept(&listener);
    holder.own_process(serde_json::from_value(held_child["identity"].clone()).unwrap());
    assert_eq!(admission(&f, None), (1, 0, 1, 1), "{label}: held permit");
    let holder_id = cli::recent(&f);

    let mut build = start(&s, &f, route, argv, &env);
    // Exactly one notice, on the wrapper's own stderr. Its reference is the
    // waiting build, which is also the newest job.
    let reference = queued_notice(&mut build, &label);
    let id = cli::recent(&f);
    assert_eq!(reference, id.as_str(), "{label}: notice reference");
    // The notice is written after the daemon queued the job: the build has a
    // queue entry, no reservation yet, and the docker stub has not started.
    assert_eq!(s.calls(), 0, "{label}: docker started under a held permit");
    assert_eq!(admission(&f, Some(&id)), (1, 1, 0, 0), "{label}: waiting");
    assert_eq!(admission(&f, None), (2, 1, 1, 1), "{label}: waiting");
    assert_eq!(
        recorded(&f, NEGOTIATED_STATUS, &id),
        container_record("LOCAL_CONTAINER"),
        "{label}: the waiting build's record"
    );
    assert!(job(&f, NEGOTIATED_STATUS, &id)["child"].is_null());
    // It waits in the Build class behind the held permit, as a local build.
    let waiting = protocol::call(
        &f,
        arc_ipc::traffic::diagnostics::OPERATION_JOB,
        json!({ "job_id": id }),
    );
    let waiting = waiting.result.expect("waiting diagnostics");
    assert_eq!(waiting["admission"]["state"], "QUEUED", "{label}");
    assert_eq!(waiting["admission"]["wait_reason"], "class_window");
    assert_eq!(waiting["classification"], container_record("HOST_LOCAL"));

    release.write_all(b"x").unwrap();
    assert_eq!(holder.wait_with_output().unwrap().status.code(), Some(17));
    let (code, stdout, stderr) = finish(&mut build, &label);
    assert_eq!(code, Some(17), "{label}: {stderr:?}");
    // Exactly one QUEUED notice was read above. A build that waited then
    // announces its start once, and only the child's own bytes follow.
    assert_eq!(
        String::from_utf8(stderr).unwrap(),
        "STARTED\nCommand started.\nfixture-stderr\n",
        "{label}"
    );
    assert_native_child(&s, argv, &stdout, &label);
    assert_eq!(s.calls(), 1, "{label}: docker must run exactly once");

    let job = job(&f, NEGOTIATED_STATUS, &id);
    assert_eq!(
        job["classification"],
        container_record("LOCAL_CONTAINER"),
        "{label}"
    );
    assert_eq!(job["outcome"]["code"], 17, "{label}: {job}");
    assert_recorded_fixture_child(&s, &job);
    assert_eq!(
        stored(&f, &id),
        ("BUILD".to_owned(), "LOCAL_CONTAINER".to_owned()),
        "{label}"
    );
    // One queue entry and one BUILD reservation, released with the child.
    assert_eq!(admission(&f, Some(&id)), (1, 0, 1, 0), "{label}: released");
    assert_eq!(admission(&f, None), (2, 0, 2, 0), "{label}: released");
    // The reservation is the Build class's share of the global budget: the
    // same amounts the held `cargo build` reserved.
    let db = rusqlite::Connection::open(&f.database).unwrap();
    let reservation = |job: &JobId| -> (f64, i64, Option<i64>) {
        db.query_row(
            "SELECT cpu_cores_est, memory_bytes_est, released_at FROM permits WHERE job_id=?1",
            [job.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
    };
    let (cpu, memory, released) = reservation(&id);
    let held = reservation(&holder_id);
    assert!(cpu > 0.0 && memory > 0, "{label}: no budget reservation");
    assert_eq!(
        (cpu, memory),
        (held.0, held.1),
        "{label}: not a BUILD share"
    );
    assert!(released.is_some(), "{label}: reservation not released");
    drop(db);
    assert_eq!(
        engine.connections(),
        0,
        "{label}: the Docker socket was contacted"
    );
    eprintln!("local container {label}: {inspection}; queue entry 1, reservation 1, stub starts 0 while held then 1");
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[cfg(target_os = "macos")]
#[test]
fn local_container_buildx_via_exec() {
    local_container_build(Route::Exec, &BUILDX);
}

#[cfg(target_os = "macos")]
#[test]
fn local_container_buildx_via_hook_exec() {
    local_container_build(Route::HookExec, &BUILDX);
}

#[cfg(target_os = "macos")]
#[test]
fn local_container_docker_build_via_exec() {
    local_container_build(Route::Exec, &BUILD);
}

#[cfg(target_os = "macos")]
#[test]
fn local_container_docker_build_via_hook_exec() {
    local_container_build(Route::HookExec, &BUILD);
}

// Remote, mixed-node, missing and conflicting endpoints keep today's result on
// both routes: no Local Container, no queue entry and no reservation.
#[test]
fn local_container_other_endpoints_unchanged() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    super::super::endpoints::install(&s);
    let mut daemon = f.spawn_isolated("test");
    let mixed = [("BUILDX_BUILDER", Some("mixed-builder"))];
    let missing = [("DOCKER_CONTEXT", Some("missing"))];
    // `local` names a Unix socket nothing proves; its builder is remote.
    let conflict = [
        ("DOCKER_CONTEXT", Some("local")),
        ("BUILDX_BUILDER", Some("remote-builder")),
    ];
    type Case<'a> = (
        &'a str,
        &'a [&'a str],
        &'a [(&'a str, Option<&'a str>)],
        &'a str,
    );
    let cases: [Case; 8] = [
        ("remote context", &BUILD, &[], "REMOTE_EXEC"),
        ("remote builder", &BUILDX, &[], "REMOTE_EXEC"),
        ("mixed-node builder", &BUILD, &mixed, "UNKNOWN"),
        ("mixed-node builder", &BUILDX, &mixed, "UNKNOWN"),
        ("missing endpoint", &BUILD, &missing, "UNKNOWN"),
        ("missing endpoint", &BUILDX, &missing, "UNKNOWN"),
        ("engine and builder conflict", &BUILD, &conflict, "UNKNOWN"),
        (
            "builder beside an unproved engine",
            &BUILDX,
            &conflict,
            "REMOTE_EXEC",
        ),
    ];
    let mut runs = 0;
    for (name, argv, env, expected) in cases {
        let inspection = s.classify(&f, argv, env);
        for route in Route::BOTH {
            let label = format!("{name} {route:?} {argv:?}");
            let (job, id) = run(&s, &f, route, argv, env);
            runs += 1;
            assert_route_outcomes(&label, &inspection, &job, expected);
            assert_eq!(job["classification"], inspection, "{label}");
            // Every earlier reader keeps the record it has today.
            assert_eq!(
                recorded(&f, OPERATION_JOB_STATUS_LOCALITY, &id),
                inspection,
                "{label}"
            );
            assert_ne!(stored(&f, &id).1, "LOCAL_CONTAINER", "{label}");
        }
        assert_eq!(admission_counts(&f), (0, 0), "{name} added local admission");
    }
    // A proven local engine whose selected builder is remote: plain `docker
    // build` cannot tell which of the two computes, and Buildx computes on its
    // remote builder. Neither becomes a Local Container.
    #[cfg(target_os = "macos")]
    {
        let engine = LocalEngine::proven(&f, &s);
        let mut env = engine.env();
        env.push(("BUILDX_BUILDER", Some("remote-builder")));
        for (argv, expected) in [(&BUILD[..], "UNKNOWN"), (&BUILDX[..], "REMOTE_EXEC")] {
            let inspection = s.classify(&f, argv, &env);
            for route in Route::BOTH {
                let label = format!("local engine, remote builder {route:?} {argv:?}");
                let (job, id) = run(&s, &f, route, argv, &env);
                runs += 1;
                assert_route_outcomes(&label, &inspection, &job, expected);
                assert_ne!(stored(&f, &id).1, "LOCAL_CONTAINER", "{label}");
            }
        }
        assert_eq!(engine.connections(), 0);
        assert_eq!(admission_counts(&f), (0, 0));
    }
    // Selectors that conflict on the command line are unresolved, and the hook
    // route admits no argv beyond the two literals: the installed hook leaves
    // such a command to the vendor's shell and `hook-exec` starts nothing.
    assert_eq!(
        installed_hook_rewrite(&s, &f, "docker --context local build ."),
        None
    );
    let conflicting = [
        "docker",
        "--context",
        "local",
        "--host",
        "tcp://192.0.2.30:2376",
        "build",
        ".",
    ];
    let inspection = s.classify(&f, &conflicting, &[]);
    let (job, _) = run(&s, &f, Route::Exec, &conflicting, &[]);
    runs += 1;
    assert_route_outcomes("conflicting selectors", &inspection, &job, "UNKNOWN");
    let mut rejected = s.command(&f, &[]);
    rejected.args(["hook-exec", "--"]).args(conflicting);
    let rejected = child_watchdog::output(&mut rejected, "conflicting selectors").unwrap();
    assert_eq!(rejected.status.code(), Some(2), "{rejected:?}");
    assert!(rejected.stdout.is_empty(), "{rejected:?}");
    let expected_runs = if cfg!(target_os = "macos") { 21 } else { 17 };
    assert_eq!((s.calls(), runs), (expected_runs, expected_runs));
    assert_eq!(admission_counts(&f), (0, 0));
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[cfg(target_os = "macos")]
#[test]
fn local_container_projection_for_earlier_peers() {
    readers::local_container_projection_for_earlier_peers();
}

#[cfg(target_os = "macos")]
#[test]
fn local_container_classify_contacts_no_socket() {
    readers::local_container_classify_contacts_no_socket();
}
