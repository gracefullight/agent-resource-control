#[test]
fn lost_committed_bun_grant_survives_old_peer_replacement() {
    use arc_ipc::{
        jobs::ProcessIdentity,
        traffic::{JobRequest, JobResult, LineageContext, OPERATION_BUN_JOB},
    };
    use arc_storage::jobs::traffic as store;
    use std::{os::unix::net::UnixListener, sync::mpsc};

    let _serial = support::serial();
    let f = Fixture::new();
    arc_storage::open_and_migrate(&f.database).unwrap();
    let listener = UnixListener::bind(&f.socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let database_path = f.database.clone();
    let (committed_tx, committed_rx) = mpsc::channel();
    let (drop_tx, drop_rx) = mpsc::channel();
    // Fault-injected peer: actual storage admission, with the socket peer PID
    // checked against the CLI observer. It drops the committed grant reply.
    // This is a fixture, not a full daemon process or an admission proxy.
    let admitting_peer = thread::spawn(move || {
        let accept = || {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                match listener.accept() {
                    Ok((stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(5)))
                            .unwrap();
                        break stream;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "admission fixture timed out");
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("admission fixture accept: {error}"),
                }
            }
        };
        let (mut probe, request) = policy_compat::next_legacy(&listener);
        assert_eq!(
            request.operation,
            arc_ipc::traffic::OPERATION_BUN_CAPABILITIES
        );
        assert!(request.payload.is_null());
        arc_ipc::write_response(
            &mut probe,
            &arc_ipc::Response::ok(
                request.request_id,
                serde_json::json!({"capability":"traffic.bun.v1"}),
            ),
        )
        .unwrap();
        drop(probe);

        let mut stream = accept();
        let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
        assert_eq!(request.operation, OPERATION_BUN_JOB);
        let input = arc_ipc::traffic::decode_job_for(&request.operation, &request.payload).unwrap();
        let JobRequest::Enqueue {
            job_id, observer, ..
        } = &input
        else {
            panic!("expected Bun enqueue")
        };
        let mut peer_pid: libc::pid_t = 0;
        let mut length = std::mem::size_of_val(&peer_pid) as libc::socklen_t;
        assert_eq!(
            unsafe {
                libc::getsockopt(
                    stream.as_raw_fd(),
                    libc::SOL_LOCAL,
                    libc::LOCAL_PEERPID,
                    (&mut peer_pid as *mut libc::pid_t).cast(),
                    &mut length,
                )
            },
            0
        );
        let (mut uid, mut gid) = (0, 0);
        assert_eq!(
            unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) },
            0
        );
        assert_eq!(uid, unsafe { libc::geteuid() });
        assert_eq!(peer_pid as u32, observer.pid);
        assert_eq!(ProcessIdentity::capture(observer.pid).unwrap(), *observer);

        let mut db = arc_storage::open_and_migrate(database_path).unwrap();
        let evidence = store::OwnerEvidence::gather(observer);
        let context = LineageContext::try_from("f".repeat(64)).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let result = store::enqueue(
            &mut db,
            &input,
            &evidence,
            &arc_traffic::Policy::default(),
            &context,
            None,
            now,
        )
        .unwrap();
        assert!(matches!(result, JobResult::Granted { .. }));
        committed_tx.send(job_id.clone()).unwrap();
        drop_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        // No response is written: admission has committed but the CLI must retry.
        drop(stream);
    });

    let (project, mut bun) = status_compat::real_bun_fixture(&f, "lost_grant_bun");
    let id = committed_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(entry(&f, bun.child.id()).unwrap().0, "RESERVED");
    assert_eq!(active(&f).1, 1);
    assert!(!project.join("started").exists());
    assert_eq!(
        unsafe { libc::kill(bun.child.id() as i32, libc::SIGSTOP) },
        0
    );
    let mut stopped = 0;
    eventually(|| unsafe {
        libc::waitpid(
            bun.child.id() as i32,
            &mut stopped,
            libc::WUNTRACED | libc::WNOHANG,
        ) > 0
            && libc::WIFSTOPPED(stopped)
    });
    drop_tx.send(()).unwrap();
    admitting_peer.join().unwrap();
    fs::remove_file(&f.socket).unwrap();

    let peer_log = f.root.join("lost-grant-old-peer-log");
    let mut old = OwnedChild::new(
        Command::new("python3")
            .args(["-c", FROZEN_OLD_PEER])
            .arg(&f.socket)
            .arg(&peer_log)
            .arg("old")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    eventually(|| f.socket.exists());
    assert_eq!(
        unsafe { libc::kill(bun.child.id() as i32, libc::SIGCONT) },
        0
    );
    eventually(|| project.join("started").exists());
    eventually(|| {
        fs::read_to_string(&peer_log)
            .unwrap_or_default()
            .contains("FALLBACK")
    });
    assert!(fs::read_to_string(project.join("stderr"))
        .unwrap()
        .contains("TRACKING_UNCERTAIN"));
    assert!(f
        .root
        .join(format!(".traffic-witness-{}", id.as_str()))
        .exists());
    assert_eq!(active(&f).1, 1);
    let requests: Vec<serde_json::Value> = fs::read_to_string(&peer_log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(requests.iter().any(|r| r["operation"] == OPERATION_BUN_JOB
        && r["payload"]["action"] == "ENQUEUE"
        && r["payload"]["job_id"] == id.as_str()));
    assert!(requests
        .iter()
        .all(|r| r["operation"] != "job.start" && r["operation"] != "traffic.job"));
    assert_eq!(
        database(&f)
            .query_row("SELECT count(*) FROM traffic_entries", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );

    old.child.kill().unwrap();
    old.child.wait().unwrap();
    fs::remove_file(&f.socket).unwrap();
    let mut restored = f.spawn("test");
    let _ = traffic(&f, &JobRequest::StatusV2 { job_id: id.clone() });
    assert_eq!(active(&f).1, 1, "a live child must keep the grant");
    fs::write(project.join("release"), "release").unwrap();
    finish(&mut bun);
    assert_eq!(invocations(&project), 1);
    eventually(|| {
        let _ = traffic(&f, &JobRequest::StatusV2 { job_id: id.clone() });
        active(&f).1 == 0
    });
    assert_eq!(
        database(&f)
            .query_row(
                "SELECT count(*) FROM permits WHERE job_id=?1",
                [id.as_str()],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    restored.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn acknowledged_bun_queue_survives_old_peer_replacement_and_original_store_recovers() {
    use arc_ipc::{jobs::JobId, traffic::JobRequest};
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let (first_path, mut first) = status_compat::real_bun_fixture(&f, "replacement_first");
    eventually(|| {
        first_path.join("started").exists()
            && entry(&f, first.child.id()).is_some_and(|e| e.0 == "RUNNING")
    });
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
    assert!(!second_path.join("started").exists());
    assert_eq!(
        unsafe { libc::kill(second.child.id() as i32, libc::SIGSTOP) },
        0
    );
    let mut stopped = 0;
    eventually(|| unsafe { libc::waitpid(second.child.id() as i32, &mut stopped, libc::WUNTRACED | libc::WNOHANG) } > 0 && libc::WIFSTOPPED(stopped));
    daemon.stop(libc::SIGTERM);
    let peer_log = f.root.join("replacement-peer-log");
    let mut old = OwnedChild::new(
        Command::new("python3")
            .args(["-c", FROZEN_OLD_PEER])
            .arg(&f.socket)
            .arg(&peer_log)
            .arg("old")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    eventually(|| f.socket.exists());
    assert_eq!(
        unsafe { libc::kill(second.child.id() as i32, libc::SIGCONT) },
        0
    );
    eventually(|| second_path.join("started").exists());
    assert!(first.child.try_wait().unwrap().is_none());
    fs::write(second_path.join("release"), "release").unwrap();
    finish(&mut second);
    fs::write(first_path.join("release"), "release").unwrap();
    finish(&mut first);
    assert_eq!(invocations(&first_path), 1);
    assert_eq!(invocations(&second_path), 1);
    assert!(f
        .root
        .join(format!(".traffic-witness-{}", id.as_str()))
        .exists());
    let requests: Vec<serde_json::Value> = fs::read_to_string(peer_log)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert!(requests.iter().any(|r| r["operation"] == "governor.job"
        && r["payload"]["request"]["action"] == "ENQUEUE"
        && r["payload"]["request"]["job_id"] == id.as_str()));
    assert!(requests
        .iter()
        .any(|r| r["operation"] == "governor.job" && r["payload"]["request"]["action"] == "FALLBACK"));
    assert!(requests
        .iter()
        .all(|r| r["operation"] != "traffic.job" && r["operation"] != "job.start"));
    assert_eq!(entry(&f, second.child.id()).unwrap().0, "QUEUED");
    assert_eq!(
        database(&f)
            .query_row(
                "SELECT count(*) FROM permits WHERE job_id=?1",
                [id.as_str()],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    old.child.kill().unwrap();
    old.child.wait().unwrap();
    fs::remove_file(&f.socket).unwrap();
    fs::rename(
        f.root.join("runtime.json"),
        f.root.join("prior-runtime.json"),
    )
    .unwrap();
    let mut restored = f.spawn("test");
    eventually(|| {
        let _ = traffic(&f, &JobRequest::StatusV2 { job_id: id.clone() });
        entry(&f, second.child.id()).is_some_and(|e| e.0 == "ABANDONED")
    });
    assert_eq!(
        database(&f)
            .query_row(
                "SELECT count(*) FROM permits WHERE job_id=?1",
                [id.as_str()],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    restored.stop(libc::SIGTERM);
    f.report();
}
