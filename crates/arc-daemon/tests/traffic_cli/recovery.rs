use super::*;

#[test]
fn replay_forged_lineage_cancellation_and_spawn_failure_preserve_one_permit() {
    use arc_ipc::{
        jobs::{JobId, ProcessIdentity},
        traffic::{JobRequest, JobResult, LineageContext},
    };
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let projects = projects(&f.root, "replay");
    let mut first = spawn_build(&f, &projects[0], true);
    eventually(|| {
        projects[0].join("started").exists()
            && entry(&f, first.child.id()).is_some_and(|entry| entry.0 == "RUNNING")
    });
    let context: String = database(&f)
        .query_row(
            "SELECT context FROM traffic_entries WHERE root_job_id IS NULL AND context IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let id = JobId::try_from("a123456789abcdef0123456789abcdef".to_owned()).unwrap();
    let classification = cargo_class("BUILD", "CARGO_BUILD");
    let request = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification,
        lineage_context: Some(LineageContext::try_from(context).unwrap()),
    };
    // Lose the acknowledgement after sending a complete frame, then retry by ID.
    let mut wire = arc_ipc::Request::new("lost", arc_ipc::traffic::OPERATION_TRAFFIC_JOB);
    wire.payload = serde_json::to_value(&request).unwrap();
    arc_ipc::write_message(&mut f.connect(), &wire).unwrap();
    let first_reply = traffic_result(traffic(&f, &request));
    let duplicate = traffic_result(traffic(&f, &request));
    assert!(matches!(first_reply, JobResult::Queued { .. }));
    assert_eq!(first_reply, duplicate);
    assert_eq!(active(&f).1, 1);
    let conflict = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification: cargo_class("TEST", "CARGO_TEST"),
        lineage_context: None,
    };
    assert_eq!(traffic(&f, &conflict).status, arc_ipc::STATUS_ERROR);
    let cancel = JobRequest::Cancel {
        job_id: id.clone(),
        observer: observer.clone(),
        permit_generation: None,
    };
    assert_eq!(
        traffic_result(traffic(&f, &cancel)),
        JobResult::Cancelled {}
    );
    assert_eq!(
        traffic_result(traffic(&f, &request)),
        JobResult::Cancelled {}
    );
    assert_eq!(active(&f).1, 1);
    // A request cannot use another live process as its observer.
    let spoof = JobRequest::Poll {
        job_id: id,
        observer: ProcessIdentity::capture(first.child.id()).unwrap(),
    };
    assert_eq!(traffic(&f, &spoof).status, arc_ipc::STATUS_ERROR);

    let mut second = spawn_build(&f, &projects[1], true);
    eventually(|| entry(&f, second.child.id()).is_some_and(|e| e.0 == "QUEUED"));
    assert_eq!(
        unsafe { libc::kill(second.child.id() as i32, libc::SIGTERM) },
        0
    );
    eventually(|| second.child.try_wait().unwrap().is_some());
    assert!(entry(&f, second.child.id()).is_some_and(|e| e.0 == "CANCELLED"));
    assert!(!projects[1].join("started").exists());
    assert_eq!(active(&f).1, 1);
    fs::write(projects[0].join("release"), "release").unwrap();
    finish(&mut first);
    let stale_context: String = database(&f)
        .query_row(
            "SELECT context FROM traffic_entries WHERE context IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let stale_id = JobId::try_from("c123456789abcdef0123456789abcdef".to_owned()).unwrap();
    let stale = JobRequest::Enqueue {
        job_id: stale_id.clone(),
        observer: observer.clone(),
        classification: cargo_class("BUILD", "CARGO_BUILD"),
        lineage_context: Some(LineageContext::try_from(stale_context).unwrap()),
    };
    let JobResult::Granted {
        permit_generation, ..
    } = traffic_result(traffic(&f, &stale))
    else {
        panic!("stale context must use normal admission")
    };
    assert_eq!(
        traffic_result(traffic(
            &f,
            &JobRequest::Cancel {
                job_id: stale_id,
                observer: observer.clone(),
                permit_generation: Some(permit_generation)
            }
        )),
        JobResult::Cancelled {}
    );
    let output = Command::new(binary("arcctl"))
        .args(["exec", "--", "cargo", "build"])
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env("PATH", &f.root)
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPAWN_FAILED"));
    assert_eq!(active(&f).1, 0);
    let db = database(&f);
    let count: i64 = db
        .query_row(
            "SELECT count(*) FROM job_lifecycle WHERE outcome_kind='SPAWN_FAILED'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn queued_nested_cli_rechecks_ancestry_and_reuses_active_lineage() {
    use arc_ipc::{
        jobs::{JobId, ProcessIdentity},
        traffic::{JobRequest, JobResult},
    };
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let root_id = JobId::try_from("f123456789abcdef0123456789abcdef".to_owned()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let root = JobRequest::Enqueue {
        job_id: root_id.clone(),
        observer: observer.clone(),
        classification: cargo_class("BUILD", "CARGO_BUILD"),
        lineage_context: None,
    };
    let JobResult::Granted {
        permit_generation, ..
    } = traffic_result(traffic(&f, &root))
    else {
        panic!("expected root grant")
    };
    symlink(binary("arc-exec-fixture"), f.root.join("cargo")).unwrap();
    let inherited = "a".repeat(64);
    let marker = f.root.join("nested-launches");
    let mut nested = OwnedChild::new(
        Command::new(binary("arcctl"))
            .args(["exec", "--", "cargo", "build"])
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env("PATH", &f.root)
            .env("ARC_TRAFFIC_LINEAGE", &inherited)
            .env("ARC_FIXTURE_MODE", "echo")
            .env("ARC_FIXTURE_MARKER", &marker)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(
                File::create(f.root.join("nested-stderr")).unwrap(),
            ))
            .spawn()
            .unwrap(),
    );
    eventually(|| entry(&f, nested.child.id()).is_some_and(|e| e.0 == "QUEUED"));
    assert!(!marker.exists());
    // Model a queued request whose previously unverified context becomes the
    // active root context. The daemon must still verify native ancestry.
    assert_eq!(
        database(&f)
            .execute(
                "UPDATE traffic_entries SET context=?1 WHERE job_id=?2",
                (&inherited, root_id.as_str())
            )
            .unwrap(),
        1
    );
    let deadline = Instant::now() + Duration::from_secs(4);
    while !marker.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let started = marker.exists();
    if !started {
        nested.child.kill().unwrap();
    }
    eventually(|| nested.child.try_wait().unwrap().is_some());
    let result = nested.child.wait().unwrap();
    let launches = fs::read_to_string(&marker)
        .map(|s| s.lines().count())
        .unwrap_or(0);
    assert_eq!(
        traffic_result(traffic(
            &f,
            &JobRequest::Cancel {
                job_id: root_id,
                observer,
                permit_generation: Some(permit_generation),
            }
        )),
        JobResult::Cancelled {}
    );
    assert_eq!(active(&f).1, 0);
    daemon.stop(libc::SIGTERM);
    f.report();
    assert!(
        started,
        "queued nested execution did not refresh its lineage proof"
    );
    assert_eq!(result.code(), Some(17));
    assert_eq!(launches, 1);
}

#[test]
fn nested_real_cargo_reuses_lineage_and_restart_keeps_the_active_child() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let projects = projects(&f.root, "nested");
    fs::write(
        projects[0].join("build.rs"),
        r#"
use std::{fs, process::Command};
fn main() {
    let inner = std::env::var("TRAFFIC_NESTED_DIR").unwrap();
    fs::write(std::env::var("TRAFFIC_TEST_MARKER").unwrap(), "started\n").unwrap();
    let status = Command::new(std::env::var("TRAFFIC_ARCCTL").unwrap())
        .args(["exec", "--", "cargo", "build", "--offline"])
        .current_dir(&inner).env("CARGO_TARGET_DIR", format!("{inner}/target"))
        .env("TRAFFIC_TEST_MARKER", format!("{inner}/started"))
        .env("TRAFFIC_TEST_RELEASE", format!("{inner}/release"))
        .status().unwrap();
    assert!(status.success());
}

"#,
    )
    .unwrap();
    let mut command = Command::new(binary("arcctl"));
    command
        .args(["exec", "--", "cargo", "build", "--offline"])
        .current_dir(&projects[0])
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env("CARGO_TARGET_DIR", projects[0].join("target"))
        .env("TRAFFIC_TEST_MARKER", projects[0].join("started"))
        .env("TRAFFIC_NESTED_DIR", &projects[1])
        .env("TRAFFIC_ARCCTL", binary("arcctl"))
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            File::create(projects[0].join("stderr")).unwrap(),
        ));
    let mut root = BuildChild::new(OwnedChild::new(command.spawn().unwrap()), &f, true);
    eventually(|| projects[1].join("started").exists());
    let db = database(&f);
    let (children, permits): (i64, i64) = db.query_row("SELECT (SELECT count(*) FROM traffic_entries WHERE root_job_id IS NOT NULL AND state='RUNNING'),(SELECT count(*) FROM permits WHERE state='ACTIVE')", [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!((children, permits), (1, 1));
    drop(db);
    daemon.stop(libc::SIGTERM);
    assert!(root.child.try_wait().unwrap().is_none());
    fs::rename(
        f.root.join("runtime.json"),
        f.root.join("prior-runtime.json"),
    )
    .unwrap();
    let mut restarted = f.spawn("test");
    assert_eq!(active(&f).1, 1);
    fs::write(projects[1].join("release"), "release").unwrap();
    finish(&mut root);
    assert_eq!(invocations(&projects[0]), 1);
    assert_eq!(invocations(&projects[1]), 1);
    assert_eq!(active(&f).1, 0);
    restarted.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn restart_recovers_gone_owner_and_child_without_replaying_work() {
    use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let projects = projects(&f.root, "recover");
    let mut wrapper = spawn_build(&f, &projects[0], true);
    eventually(|| {
        projects[0].join("started").exists()
            && entry(&f, wrapper.child.id()).is_some_and(|e| e.1.is_some())
    });
    let cargo =
        ProcessIdentity::capture(entry(&f, wrapper.child.id()).unwrap().1.unwrap()).unwrap();
    wrapper.child.kill().unwrap();
    wrapper.child.wait().unwrap();
    assert_eq!(probe_live(&cargo), IdentityStatus::VerifiedMatch);
    fs::write(projects[0].join("release"), "release").unwrap();
    eventually(|| {
        matches!(
            probe_live(&cargo),
            IdentityStatus::Missing | IdentityStatus::Mismatch
        )
    });
    daemon.stop(libc::SIGTERM);
    fs::rename(
        f.root.join("runtime.json"),
        f.root.join("prior-runtime.json"),
    )
    .unwrap();
    let mut restarted = f.spawn("test");
    assert!(entry(&f, wrapper.child.id())
        .is_some_and(|entry| entry.0 == "ABANDONED" && entry.4.is_some()));
    assert_eq!(active(&f).1, 0);
    assert_eq!(invocations(&projects[0]), 1);
    restarted.stop(libc::SIGTERM);
    f.report();
}

pub(super) fn spawn_cached_build(f: &Fixture, project: &Path, stderr: &Path) -> BuildChild {
    let mut command = Command::new(binary("arcctl"));
    observe_cargo(&mut command, f, project);
    let child = command
        .args(["exec", "--", "cargo", "build", "--offline", "--verbose"])
        .current_dir(project)
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env("CARGO_TARGET_DIR", project.join("target"))
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(File::create(stderr).unwrap()))
        .spawn()
        .unwrap();
    BuildChild::new(OwnedChild::new(child), f, true)
}

#[test]
fn cached_cargo_launch_still_executes_one_native_child() {
    use std::collections::BTreeSet;
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    install_cargo_observer(&f);
    let project = projects(&f.root, "cached")[0].clone();
    fs::remove_file(project.join("build.rs")).unwrap();
    for iteration in 0..2 {
        let stderr = project.join(format!("cache-{iteration}.stderr"));
        let mut wrapper = spawn_cached_build(&f, &project, &stderr);
        let mut observed = BTreeSet::new();
        let deadline = Instant::now() + Duration::from_secs(45);
        loop {
            for child in children(wrapper.child.id()) {
                if cargo_pid(child) {
                    observed.insert(child);
                }
            }
            if let Some(status) = wrapper.child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        let launches = cargo_launches(&project);
        assert_eq!(launches.len(), iteration + 1);
        assert!(observed.len() <= 1);
        assert!(observed.iter().all(|pid| *pid == launches[iteration]));
        let recorded: u32 = database(&f)
            .query_row(
                "SELECT child_pid FROM job_lifecycle WHERE observer_pid=?1",
                [wrapper.child.id()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(launches[iteration], recorded);
        if iteration == 1 {
            assert!(fs::read_to_string(stderr)
                .unwrap()
                .contains("Fresh traffic_fixture_cached_0"));
        }
    }
    let db = database(&f);
    let (jobs, permits): (i64, i64) = db.query_row("SELECT (SELECT count(*) FROM traffic_entries WHERE state='COMPLETED'),(SELECT count(*) FROM permits WHERE state='RELEASED')", [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!((jobs, permits), (2, 2));
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}
