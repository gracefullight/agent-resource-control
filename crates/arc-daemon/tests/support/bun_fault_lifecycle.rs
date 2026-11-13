// Spawned by the test below: this executes the normal daemon entry point in a
// separate test-binary process, with a cfg(test)-only lost-reply hook.
#[test]
fn bun_fault_daemon_child() {
    if std::env::var_os("ARC_TEST_BUN_DAEMON_CHILD").is_some() {
        assert_eq!(crate::main(), std::process::ExitCode::SUCCESS);
    }
}

#[test]
fn committed_bun_grant_lost_before_old_peer_replacement_recovers_original_store() {
    use arc_ipc::governor::OPERATION_JOB;
    use std::{os::unix::fs::DirBuilderExt, path::Path, process::Child};

    struct Owned(Child);
    impl Drop for Owned {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = self.0.kill();
            }
            let _ = self.0.wait();
        }
    }
    struct Release(std::path::PathBuf);
    impl Drop for Release {
        fn drop(&mut self) {
            let _ = fs::write(&self.0, "release");
        }
    }
    fn until(mut predicate: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !predicate() {
            assert!(
                Instant::now() < deadline,
                "full daemon fault fixture timed out"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn stop(daemon: &mut Owned) {
        assert_eq!(
            unsafe { libc::kill(daemon.0.id() as i32, libc::SIGTERM) },
            0
        );
        until(|| daemon.0.try_wait().unwrap().is_some());
        assert!(daemon.0.wait().unwrap().success());
    }
    fn daemon(root: &Path, marker: Option<&Path>, suffix: &str) -> Owned {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "lifecycle_tests::bun_fault_daemon_child",
                "--nocapture",
            ])
            .env("ARC_TEST_BUN_DAEMON_CHILD", "1")
            .env(arc_core::ARC_DB_PATH, root.join("db"))
            .env(arc_core::ARC_SOCKET_PATH, root.join("sock"))
            .env("ARC_IPC_PROFILE", "test")
            .env(
                "ARC_RUNTIME_REPORT",
                root.join(format!("runtime-{suffix}.json")),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::from(
                fs::File::create(root.join(format!("daemon-{suffix}.out"))).unwrap(),
            ))
            .stderr(Stdio::from(
                fs::File::create(root.join(format!("daemon-{suffix}.err"))).unwrap(),
            ));
        if let Some(marker) = marker {
            command.env("ARC_TEST_BUN_GRANT_ACK_LOST", marker);
        } else {
            command.env_remove("ARC_TEST_BUN_GRANT_ACK_LOST");
        }
        let mut child = Owned(command.spawn().unwrap());
        until(|| {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "test daemon exited before binding"
            );
            root.join("sock").exists()
        });
        child
    }
    fn status(bin: &Path, root: &Path, id: &str) {
        let _ = Command::new(bin.join("arcctl"))
            .args(["job", "status", id])
            .env(arc_core::ARC_SOCKET_PATH, root.join("sock"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
    }
    fn active(root: &Path) -> i64 {
        rusqlite::Connection::open(root.join("db"))
            .unwrap()
            .query_row(
                "SELECT count(*) FROM permits WHERE state='ACTIVE'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    }

    // Frozen old dispatch fixture. The accepted protocol-1 dispatcher at
    // 16772fe2aa350413225f3fb2bb9227d14ae5a6c4 rejects unknown operations
    // before decoding their payload; no admission request is proxied.
    const OLD_PEER: &str = r#"
import json, socket, struct, sys
path, log = sys.argv[1:]
listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
listener.bind(path); listener.listen(8)
def readn(conn, n):
    out = b''
    while len(out) < n:
        block = conn.recv(n-len(out))
        if not block: raise EOFError()
        out += block
    return out
while True:
    conn, _ = listener.accept()
    with conn:
        try:
            length = struct.unpack('>I', readn(conn, 4))[0]
            request = json.loads(readn(conn, length))
            with open(log, 'a') as output: output.write(json.dumps(request) + '\n')
            response = {'protocol_version':1, 'request_id':request['request_id'], 'status':'error', 'error':{'code':'UNSUPPORTED_CAPABILITY'}}
            data = json.dumps(response).encode()
            conn.sendall(struct.pack('>I', len(data)) + data)
        except (EOFError, BrokenPipeError, ConnectionResetError): pass
"#;

    let _serial = support::serial();
    let bin = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    for cancel in [false, true] {
        let root = arc_core::runtime_root().unwrap().join(format!(
            "bun-lost-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        eprintln!("lost-grant fixture={} cancel={cancel}", root.display());
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let project = root.join("bun");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("native.test.ts"), "import {test} from 'bun:test'; import {appendFileSync,existsSync} from 'node:fs'; test('once', async () => { appendFileSync(process.env.TEST_MARKER!, 'once\\n'); while (!existsSync(process.env.TEST_RELEASE!)) await Bun.sleep(10); }, 60000);\n").unwrap();
        let release = project.join("release");
        let _release = Release(release.clone());
        let ack_marker = root.join("grant-committed");
        let mut first = daemon(&root, Some(&ack_marker), "first");
        let mut cli = Owned(
            Command::new(bin.join("arcctl"))
                .args(["exec", "--", "bun", "test", "--timeout=60000"])
                .current_dir(&project)
                .env(arc_core::ARC_SOCKET_PATH, root.join("sock"))
                .env("TEST_MARKER", project.join("started"))
                .env("TEST_RELEASE", &release)
                .env_remove("ARC_TRAFFIC_LINEAGE")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::from(
                    fs::File::create(project.join("stderr")).unwrap(),
                ))
                .spawn()
                .unwrap(),
        );
        until(|| ack_marker.exists());
        let mut stopped = 0;
        until(|| unsafe { libc::waitpid(cli.0.id() as i32, &mut stopped, libc::WUNTRACED | libc::WNOHANG) } > 0 && libc::WIFSTOPPED(stopped));
        let db = rusqlite::Connection::open(root.join("db")).unwrap();
        let (id, state): (String, String) = db.query_row(
            "SELECT t.job_id,t.state FROM traffic_entries t JOIN job_lifecycle l ON l.job_id=t.job_id WHERE l.observer_pid=?1",
            [cli.0.id()], |r| Ok((r.get(0)?,r.get(1)?)),
        ).unwrap();
        assert_eq!(state, "RESERVED");
        assert_eq!(active(&root), 1);
        drop(db);
        assert!(!project.join("started").exists());
        stop(&mut first);

        let peer_log = root.join("old-peer-log");
        let mut old = Owned(
            Command::new("python3")
                .args(["-c", OLD_PEER])
                .arg(root.join("sock"))
                .arg(&peer_log)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::from(
                    fs::File::create(root.join("old-peer.err")).unwrap(),
                ))
                .spawn()
                .unwrap(),
        );
        until(|| root.join("sock").exists());
        if cancel {
            assert_eq!(unsafe { libc::kill(cli.0.id() as i32, libc::SIGTERM) }, 0);
        }
        assert_eq!(unsafe { libc::kill(cli.0.id() as i32, libc::SIGCONT) }, 0);
        if cancel {
            until(|| cli.0.try_wait().unwrap().is_some());
            assert!(!cli.0.wait().unwrap().success());
            assert!(!project.join("started").exists());
        } else {
            until(|| project.join("started").exists());
            until(|| {
                fs::read_to_string(&peer_log)
                    .unwrap_or_default()
                    .contains("FALLBACK")
            });
            assert!(fs::read_to_string(project.join("stderr"))
                .unwrap()
                .contains("TRACKING_UNCERTAIN"));
        }
        assert!(root.join(format!(".traffic-witness-{id}")).exists());
        assert_eq!(active(&root), 1);
        let requests: Vec<serde_json::Value> = fs::read_to_string(&peer_log)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(requests
            .iter()
            .all(|r| r["operation"] == OPERATION_JOB
                && r["payload"]["action"] == "TRAFFIC"
                && r["payload"]["request"]["job_id"] == id));
        if !cancel {
            assert!(requests.iter().any(|r| r["payload"]["request"]["action"] == "ENQUEUE"));
            assert!(requests
                .iter()
                .any(|r| r["payload"]["request"]["action"] == "FALLBACK"));
        }
        old.0.kill().unwrap();
        old.0.wait().unwrap();
        fs::remove_file(root.join("sock")).unwrap();
        let mut restored = daemon(&root, None, "restored");
        status(&bin, &root, &id);
        if !cancel {
            assert_eq!(
                active(&root),
                1,
                "live Bun child retains the original grant"
            );
            fs::write(&release, "release").unwrap();
            until(|| cli.0.try_wait().unwrap().is_some());
            assert!(cli.0.wait().unwrap().success());
            assert_eq!(
                fs::read_to_string(project.join("started")).unwrap(),
                "once\n"
            );
        }
        until(|| {
            status(&bin, &root, &id);
            active(&root) == 0
        });
        let db = rusqlite::Connection::open(root.join("db")).unwrap();
        let counts: (i64, i64) = db.query_row(
            "SELECT (SELECT count(*) FROM traffic_entries WHERE job_id=?1), (SELECT count(*) FROM permits WHERE job_id=?1)",
            [&id], |r| Ok((r.get(0)?,r.get(1)?)),
        ).unwrap();
        assert_eq!(counts, (1, 1));
        let job = arc_storage::jobs::get(
            &db,
            &arc_ipc::jobs::JobId::try_from(id.clone()).unwrap(),
        )
        .unwrap()
        .unwrap();
        // Outage completion can persist the wrapper's observed outcome without
        // a live child attachment. The old peer's captured FALLBACK and native
        // marker independently identify the one child; never invent an attach.
        let children: Vec<arc_core::process_identity::ProcessIdentity> = requests
            .iter()
            .filter_map(|r| {
                let child = &r["payload"]["request"]["child"];
                (!child.is_null()).then(|| serde_json::from_value(child.clone()).unwrap())
            })
            .collect();
        assert_eq!(children.len(), usize::from(!cancel));
        for child in &children {
            assert_ne!(
                arc_core::process_identity::probe_live(child),
                arc_core::process_identity::IdentityStatus::VerifiedMatch
            );
        }
        if !cancel {
            assert_eq!(job.outcome, Some(arc_ipc::jobs::Outcome::Exited { code: 0 }));
        }
        eprintln!(
            "lost-grant-reconciled={}",
            serde_json::json!({"cancel":cancel,"jobId":id,"nativeChildren":children,"sqliteChild":job.child,
                "outcome":job.outcome,"entries":counts.0,"permits":counts.1,
                "activePermits":active(&root),"targetExecutions":usize::from(!cancel),
                "wrapperSettled":true,"originalStoreRecovered":true})
        );
        drop(db);
        stop(&mut restored);
        fs::remove_dir_all(root).unwrap();
    }
}
