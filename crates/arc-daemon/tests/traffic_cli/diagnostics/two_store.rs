fn spawn_other_store(f: &Fixture, database: &Path) -> OwnedChild {
    let mut command = f.command();
    command
        .env(arc_core::ARC_DB_PATH, database)
        .env("ARC_IPC_PROFILE", "test");
    let mut child = OwnedChild::new(command.spawn().unwrap());
    f.wait_for(&mut child, "arc-daemon ready.");
    child
}

#[test]
fn lost_cargo_enqueue_then_replacement_fallback_keeps_original_witness() {
    use arc_ipc::{
        jobs::ProcessIdentity,
        traffic::{JobRequest, JobResult, LineageContext},
    };
    use arc_storage::jobs::traffic as store;
    use std::{os::unix::net::UnixListener, sync::mpsc};

    let _serial = support::serial();
    let f = Fixture::new();
    arc_storage::open_and_migrate(&f.database).unwrap();
    let listener = UnixListener::bind(&f.socket).unwrap();
    let original = f.database.clone();
    let (commit_tx, commit_rx) = mpsc::channel();
    let (disconnect_tx, disconnect_rx) = mpsc::channel();
    // This fault peer commits to the original store and drops the enqueue
    // acknowledgement while the wrapper is stopped. It never serves retries.
    let admitting = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
        assert_eq!(
            request.operation,
            arc_ipc::traffic::OPERATION_TRAFFIC_CAPABILITIES
        );
        arc_ipc::write_response(
            &mut stream,
            &arc_ipc::Response::ok(
                request.request_id,
                serde_json::json!({"capability":"traffic.v1"}),
            ),
        )
        .unwrap();
        drop(stream);
        let (mut stream, _) = listener.accept().unwrap();
        let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
        assert_eq!(request.operation, arc_ipc::traffic::OPERATION_TRAFFIC_JOB);
        let input = arc_ipc::traffic::decode_job_for(&request.operation, &request.payload).unwrap();
        let JobRequest::Enqueue {
            job_id,
            observer,
            classification,
            ..
        } = &input
        else {
            panic!("expected enqueue")
        };
        assert_eq!(*classification, cargo_class("TEST", "CARGO_TEST"));
        let mut pid: libc::pid_t = 0;
        let mut length = std::mem::size_of_val(&pid) as libc::socklen_t;
        assert_eq!(
            unsafe {
                libc::getsockopt(
                    stream.as_raw_fd(),
                    libc::SOL_LOCAL,
                    libc::LOCAL_PEERPID,
                    (&mut pid as *mut libc::pid_t).cast(),
                    &mut length,
                )
            },
            0
        );
        assert_eq!(pid as u32, observer.pid);
        assert_eq!(ProcessIdentity::capture(observer.pid).unwrap(), *observer);
        let mut db = arc_storage::open_and_migrate(original).unwrap();
        let context = LineageContext::try_from("e".repeat(64)).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert!(matches!(
            store::enqueue(
                &mut db,
                &input,
                &store::OwnerEvidence::gather(observer),
                &arc_traffic::Policy::default(),
                &context,
                None,
                now,
            )
            .unwrap(),
            JobResult::Granted { .. }
        ));
        commit_tx.send(job_id.clone()).unwrap();
        disconnect_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        drop(stream);
        drop(listener);
    });

    let [project, _] = projects(&f.root, "cargo_two_store");
    fs::write(
        project.join("src/lib.rs"),
        r#"#[test]
fn runs_once() {
    use std::io::Write;
    let path = std::env::var_os("TRAFFIC_RUN_LOG").unwrap();
    writeln!(std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap(), "run").unwrap();
}
"#,
    )
    .unwrap();
    install_cargo_observer(&f);
    let mut command = Command::new(binary("arcctl"));
    command.args(["exec", "--", "cargo", "test", "--offline"]);
    observe_cargo(&mut command, &f, &project);
    command
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env("CARGO_TARGET_DIR", project.join("target"))
        .env("TRAFFIC_TEST_MARKER", project.join("started"))
        .env("TRAFFIC_TEST_RELEASE", project.join("release"))
        .env("TRAFFIC_RUN_LOG", project.join("test-runs"))
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .current_dir(&project)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(File::create(project.join("stderr")).unwrap()));
    let mut cargo = BuildChild::new(OwnedChild::new(command.spawn().unwrap()), &f, true);
    let id = commit_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(active(&f).1, 1);
    assert_eq!(
        unsafe { libc::kill(cargo.child.id() as i32, libc::SIGSTOP) },
        0
    );
    let mut stopped = 0;
    eventually(|| unsafe { libc::waitpid(cargo.child.id() as i32, &mut stopped, libc::WUNTRACED | libc::WNOHANG) } > 0 && libc::WIFSTOPPED(stopped));
    disconnect_tx.send(()).unwrap();
    admitting.join().unwrap();
    fs::remove_file(&f.socket).unwrap();
    assert_eq!(
        unsafe { libc::kill(cargo.child.id() as i32, libc::SIGCONT) },
        0
    );

    // A native start while the socket is absent proves the CLI entered its
    // uncertain fail-open path before the replacement daemon appears.
    eventually(|| project.join("started").exists());
    assert_eq!(
        unsafe { libc::kill(cargo.child.id() as i32, libc::SIGSTOP) },
        0
    );
    eventually(|| unsafe { libc::waitpid(cargo.child.id() as i32, &mut stopped, libc::WUNTRACED | libc::WNOHANG) } > 0 && libc::WIFSTOPPED(stopped));
    let replacement_db = f.root.join("replacement.sqlite3");
    let mut replacement = spawn_other_store(&f, &replacement_db);
    fs::write(project.join("release"), "release").unwrap();
    assert_eq!(
        unsafe { libc::kill(cargo.child.id() as i32, libc::SIGCONT) },
        0
    );
    finish(&mut cargo);
    assert_eq!(cargo_launches(&project).len(), 1);
    assert_eq!(invocations(&project), 1);
    assert_eq!(
        fs::read_to_string(project.join("test-runs"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert!(fs::read_to_string(project.join("stderr"))
        .unwrap()
        .contains("TRACKING_UNCERTAIN"));
    let replacement_conn = rusqlite::Connection::open(&replacement_db).unwrap();
    let replacement_state: String = replacement_conn
        .query_row(
            "SELECT terminal_state FROM traffic_tombstones WHERE job_id=?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(replacement_state, "ABANDONED");
    drop(replacement_conn);
    let witness = f.root.join(format!(".traffic-witness-{}", id.as_str()));
    assert!(
        witness.exists(),
        "replacement fallback must retain original-store evidence"
    );
    assert_eq!(active(&f).1, 1);

    replacement.stop(libc::SIGTERM);
    fs::rename(
        f.root.join("runtime.json"),
        f.root.join("replacement-runtime.json"),
    )
    .unwrap();
    let mut restored = f.spawn("test");
    eventually(|| {
        let _ = traffic(&f, &JobRequest::StatusV2 { job_id: id.clone() });
        active(&f).1 == 0
    });
    restored.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn lost_bun_grant_then_successful_retry_against_another_store_keeps_original_witness() {
    use arc_ipc::{
        jobs::ProcessIdentity,
        traffic::{JobRequest, JobResult, LineageContext},
    };
    use arc_storage::jobs::traffic as store;
    use std::{os::unix::net::UnixListener, sync::mpsc};

    let _serial = support::serial();
    let f = Fixture::new();
    arc_storage::open_and_migrate(&f.database).unwrap();
    let listener = UnixListener::bind(&f.socket).unwrap();
    let original = f.database.clone();
    let (commit_tx, commit_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    // Fault fixture executes the real storage admission with the actual CLI
    // peer PID, then deliberately loses the committed response.
    let admitting = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
        assert_eq!(
            request.operation,
            arc_ipc::traffic::OPERATION_BUN_CAPABILITIES
        );
        arc_ipc::write_response(
            &mut stream,
            &arc_ipc::Response::ok(
                request.request_id,
                serde_json::json!({"capability":"traffic.bun.v1"}),
            ),
        )
        .unwrap();
        drop(stream);
        let (mut stream, _) = listener.accept().unwrap();
        let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
        let input = arc_ipc::traffic::decode_job_for(&request.operation, &request.payload).unwrap();
        let JobRequest::Enqueue {
            job_id, observer, ..
        } = &input
        else {
            panic!("expected enqueue")
        };
        let mut pid: libc::pid_t = 0;
        let mut length = std::mem::size_of_val(&pid) as libc::socklen_t;
        assert_eq!(
            unsafe {
                libc::getsockopt(
                    stream.as_raw_fd(),
                    libc::SOL_LOCAL,
                    libc::LOCAL_PEERPID,
                    (&mut pid as *mut libc::pid_t).cast(),
                    &mut length,
                )
            },
            0
        );
        assert_eq!(pid as u32, observer.pid);
        assert_eq!(ProcessIdentity::capture(observer.pid).unwrap(), *observer);
        let mut db = arc_storage::open_and_migrate(original).unwrap();
        let context = LineageContext::try_from("f".repeat(64)).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert!(matches!(
            store::enqueue(
                &mut db,
                &input,
                &store::OwnerEvidence::gather(observer),
                &arc_traffic::Policy::default(),
                &context,
                None,
                now
            )
            .unwrap(),
            JobResult::Granted { .. }
        ));
        commit_tx.send(job_id.clone()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        drop(stream);
    });
    let (project, mut bun) = status_compat::real_bun_fixture(&f, "two_store_lost");
    let id = commit_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(active(&f).1, 1);
    assert_eq!(
        unsafe { libc::kill(bun.child.id() as i32, libc::SIGSTOP) },
        0
    );
    let mut stopped = 0;
    eventually(|| unsafe { libc::waitpid(bun.child.id() as i32, &mut stopped, libc::WUNTRACED | libc::WNOHANG) } > 0 && libc::WIFSTOPPED(stopped));
    release_tx.send(()).unwrap();
    admitting.join().unwrap();
    fs::remove_file(&f.socket).unwrap();
    let second_db = f.root.join("replacement.sqlite3");
    let mut replacement = spawn_other_store(&f, &second_db);
    assert_eq!(
        unsafe { libc::kill(bun.child.id() as i32, libc::SIGCONT) },
        0
    );
    eventually(|| project.join("started").exists());
    let second = rusqlite::Connection::open(&second_db).unwrap();
    assert_eq!(
        second
            .query_row(
                "SELECT count(*) FROM permits WHERE job_id=?1 AND state='ACTIVE'",
                [id.as_str()],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    drop(second);
    fs::write(project.join("release"), "release").unwrap();
    finish(&mut bun);
    assert_eq!(invocations(&project), 1);
    let diagnostics = fs::read_to_string(project.join("stderr")).unwrap();
    assert!(diagnostics.contains("TRACKING_UNCERTAIN"));
    assert!(diagnostics.contains("COMPLETION_UNCONFIRMED"));
    let witness = f.root.join(format!(".traffic-witness-{}", id.as_str()));
    assert!(
        witness.exists(),
        "a replacement-store completion cannot discard original evidence"
    );
    assert_eq!(active(&f).1, 1);
    replacement.stop(libc::SIGTERM);
    fs::rename(
        f.root.join("runtime.json"),
        f.root.join("replacement-runtime.json"),
    )
    .unwrap();
    let mut restored = f.spawn("test");
    eventually(|| {
        let _ = traffic(&f, &JobRequest::StatusV2 { job_id: id.clone() });
        active(&f).1 == 0
    });
    restored.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn acknowledged_bun_queue_then_new_store_grant_keeps_original_witness() {
    use arc_ipc::{jobs::JobId, traffic::JobRequest};
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let (first_path, mut first) = status_compat::real_bun_fixture(&f, "replacement_first");
    eventually(|| {
        first_path.join("started").exists()
            && entry(&f, first.child.id()).is_some_and(|e| e.0 == "RUNNING")
    });
    // This valid caller-supplied value forces idempotent re-enqueue when queued.
    let (second_path, mut second) = status_compat::real_bun_fixture(&f, "replacement_second");
    eventually(|| {
        entry(&f, second.child.id()).is_some_and(|e| e.0 == "QUEUED")
            && fs::read_to_string(second_path.join("stderr"))
                .unwrap()
                .contains("QUEUED")
    });
    let id = JobId::try_from(
        database(&f)
            .query_row(
                "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1",
                [second.child.id()],
                |r| r.get::<_, String>(0),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        unsafe { libc::kill(second.child.id() as i32, libc::SIGSTOP) },
        0
    );
    let mut stopped = 0;
    eventually(|| unsafe { libc::waitpid(second.child.id() as i32, &mut stopped, libc::WUNTRACED | libc::WNOHANG) } > 0 && libc::WIFSTOPPED(stopped));
    daemon.stop(libc::SIGTERM);
    fs::rename(
        f.root.join("runtime.json"),
        f.root.join("original-runtime.json"),
    )
    .unwrap();
    let second_db = f.root.join("replacement.sqlite3");
    let mut replacement = spawn_other_store(&f, &second_db);
    assert_eq!(
        unsafe { libc::kill(second.child.id() as i32, libc::SIGCONT) },
        0
    );
    eventually(|| second_path.join("started").exists());
    assert!(first.child.try_wait().unwrap().is_none());
    fs::write(second_path.join("release"), "release").unwrap();
    finish(&mut second);
    assert_eq!(invocations(&second_path), 1);
    let diagnostics = fs::read_to_string(second_path.join("stderr")).unwrap();
    assert!(diagnostics.contains("TRACKING_UNCERTAIN"));
    assert!(diagnostics.contains("COMPLETION_UNCONFIRMED"));
    assert_eq!(entry(&f, second.child.id()).unwrap().0, "QUEUED");
    assert!(f
        .root
        .join(format!(".traffic-witness-{}", id.as_str()))
        .exists());
    let second_store = rusqlite::Connection::open(&second_db).unwrap();
    assert_eq!(
        second_store
            .query_row(
                "SELECT count(*) FROM permits WHERE job_id=?1 AND state='RELEASED'",
                [id.as_str()],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    drop(second_store);
    replacement.stop(libc::SIGTERM);
    fs::rename(
        f.root.join("runtime.json"),
        f.root.join("replacement-runtime.json"),
    )
    .unwrap();
    let mut restored = f.spawn("test");
    eventually(|| {
        let _ = traffic(&f, &JobRequest::StatusV2 { job_id: id.clone() });
        entry(&f, second.child.id()).is_some_and(|e| e.0 == "ABANDONED")
    });
    fs::write(first_path.join("release"), "release").unwrap();
    finish(&mut first);
    assert_eq!(invocations(&first_path), 1);
    eventually(|| active(&f).1 == 0);
    restored.stop(libc::SIGTERM);
    f.report();
}
