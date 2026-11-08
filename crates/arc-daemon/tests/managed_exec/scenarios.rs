use super::{
    cli::{binary, child_command, command, recent},
    protocol,
    runtime::Fixture,
    support,
};
use std::{
    fs,
    os::unix::{fs::symlink, process::CommandExt},
    process::Stdio,
};
#[test]
fn executable_with_spaces_closed_descriptors_and_redirected_streams_match_direct() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let program = f.root.join("native tool with spaces");
    symlink(binary("arc-exec-fixture"), &program).unwrap();
    for closed in [None, Some(0), Some(1), Some(2)] {
        let mut results = Vec::new();
        for managed in [false, true] {
            let mut c = if managed {
                let mut c = command(&f);
                c.args(["exec", "--"]).arg(&program);
                c
            } else {
                std::process::Command::new(&program)
            };
            c.env("ARC_FIXTURE_MODE", "echo")
                .current_dir(&f.root)
                .stdin(Stdio::null());
            if let Some(fd) = closed {
                unsafe {
                    c.pre_exec(move || {
                        libc::close(fd);
                        Ok(())
                    });
                }
            }
            results.push(c.output().unwrap());
        }
        assert_eq!(
            results[0].status.code(),
            Some(17),
            "closed descriptor {closed:?}"
        );
        assert_eq!(results[0].status, results[1].status);
        assert_eq!(results[0].stdout, results[1].stdout);
        assert_eq!(results[0].stderr, results[1].stderr);
    }
    let mut outputs = Vec::new();
    for managed in [false, true] {
        let file = f.root.join(format!("redirect-{managed}"));
        let out = child_command(&f, managed)
            .stdin(Stdio::null())
            .stdout(fs::File::create(&file).unwrap())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(17));
        assert!(out.stdout.is_empty());
        outputs.push(fs::read(file).unwrap());
    }
    assert_eq!(outputs[0], outputs[1]);
    for args in [
        vec!["exec", "--", ""],
        vec!["exec", "--", "--bad-literal-program"],
    ] {
        let out = command(&f).args(&args).output().unwrap();
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&out.stderr).contains("bad-literal"));
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}
#[test]
fn status_discovery_labels_and_missing_id_never_launch() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let out = command(&f).args(["job", "status"]).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("No lifecycle records."));
    for id in ["private-secret-id", "0000000000000000000000000000ffffff"] {
        let out = command(&f).args(["job", "status", id]).output().unwrap();
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
        assert_eq!(
            out.stderr,
            b"JOB_NOT_FOUND\nNo lifecycle record was found for this ID.\n"
        );
    }
    for args in [
        vec!["job", "private-secret-subcommand"],
        vec!["job", "status", "--private-secret"],
        vec!["job", "status", "a", "private-secret-extra"],
    ] {
        let out = command(&f).args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
        assert_eq!(
            out.stderr,
            b"INVALID_REQUEST\nUse arcctl job status [JOB_ID]. No work was launched.\n"
        );
    }
    protocol::start(&f, &protocol::register(90));
    let out = command(&f)
        .args(["job", "status", recent(&f).as_str()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("State: starting"));
    assert!(text.contains("Identity: unavailable"));
    let out = command(&f)
        .args(["exec", "--"])
        .arg(&f.root)
        .output()
        .unwrap();
    assert_eq!(
        out.stderr,
        b"SPAWN_FAILED\nNo child was started. Check the executable and execution permissions.\n"
    );
    let out = command(&f)
        .args(["job", "status", recent(&f).as_str()])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("State: spawn failed"));
    let db = rusqlite::Connection::open(&f.database).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM job_lifecycle", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    // Controlled valid timestamps prove registration and update are separate
    // fields, and keep each summary associated with its own opaque ID.
    let failed_id = recent(&f);
    for (id, registered, updated) in [
        (protocol::id(90), 100_i64, 4_000_000_000_350_i64),
        (failed_id.clone(), 200_i64, 4_000_000_000_475_i64),
    ] {
        assert_eq!(
            db.execute(
                "UPDATE job_lifecycle SET registered_at_ms=?1, updated_at_ms=?2 WHERE job_id=?3",
                rusqlite::params![registered, updated, id.as_str()],
            )
            .unwrap(),
            1
        );
    }
    drop(db);
    let response = protocol::call(
        &f,
        arc_ipc::jobs::OPERATION_JOB_STATUS,
        serde_json::json!(arc_ipc::jobs::StatusRequest::Recent {}),
    );
    assert_eq!(response.status, "ok");
    let summaries: arc_ipc::jobs::StatusResult =
        serde_json::from_value(response.result.unwrap()).unwrap();
    let arc_ipc::jobs::StatusResult::Recent {
        jobs,
        truncated,
        liveness_fresh,
        ..
    } = summaries
    else {
        panic!("expected recent summaries");
    };
    assert!(!truncated);
    assert!(!liveness_fresh);
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0].job_id, failed_id);
    assert_eq!(jobs[1].job_id, protocol::id(90));
    assert_eq!(
        (jobs[0].registered_at_ms, jobs[0].updated_at_ms),
        (200, 4_000_000_000_475)
    );
    assert_eq!(
        (jobs[1].registered_at_ms, jobs[1].updated_at_ms),
        (100, 4_000_000_000_350)
    );
    assert_eq!(
        jobs[0].reason,
        arc_ipc::jobs::LifecycleReason::ObservedSpawnFailure
    );
    assert_eq!(
        jobs[1].reason,
        arc_ipc::jobs::LifecycleReason::RegisteredNotClaimed
    );
    let out = command(&f).args(["job", "status"]).output().unwrap();
    assert!(out.status.success());
    assert!(out.stderr.is_empty());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!(
            "Job: {}\nState: spawn failed\nRegistered (Unix ms): 200\nUpdated (Unix ms): 4000000000475\nReason: OBSERVED_SPAWN_FAILURE\nLast known: true\nJob: {}\nState: starting\nRegistered (Unix ms): 100\nUpdated (Unix ms): 4000000000350\nReason: REGISTERED_NOT_CLAIMED\nLast known: true\nScheduling: not implemented\n",
            failed_id.as_str(), protocol::id(90).as_str()
        )
    );
    let db = rusqlite::Connection::open(&f.database).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM job_lifecycle", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2,
        "status must never launch work"
    );
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn native_non_rust_child_observes_literal_closed_standard_descriptors() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let source = r#"import fcntl,json,sys
state=[]
for fd in range(3):
    try:
        fcntl.fcntl(fd,fcntl.F_GETFD)
        state.append(True)
    except OSError:
        state.append(False)
with open(sys.argv[1],'w') as report:
    json.dump(state,report)
sys.exit(17)
"#;
    for mask in [1u8, 2, 4, 7] {
        for managed in [false, true] {
            let report = f.root.join(format!("native-fds-{mask}-{managed}.json"));
            let mut c = if managed {
                let mut c = command(&f);
                c.args(["exec", "--", "python3"]);
                c
            } else {
                std::process::Command::new("python3")
            };
            c.args(["-c", source]).arg(&report).stdin(Stdio::null());
            unsafe {
                c.pre_exec(move || {
                    for fd in 0..3 {
                        if mask & (1 << fd) != 0 {
                            libc::close(fd);
                        }
                    }
                    Ok(())
                });
            }
            let output = c.output().unwrap();
            assert_eq!(output.status.code(), Some(17), "{output:?}");
            assert!(output.stdout.is_empty());
            assert!(output.stderr.is_empty());
            let actual: Vec<bool> = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
            assert_eq!(
                actual,
                (0..3).map(|fd| mask & (1 << fd) == 0).collect::<Vec<_>>(),
                "closed mask {mask}, managed {managed}"
            );
        }
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}
