#[test]
fn independently_executed_old_peer_gets_only_legacy_lifecycle_on_bun_fallback() {
    let _serial = support::serial();
    for mode in ["old", "replace"] {
        let f = Fixture::new();
        let log = f.root.join("old-peer-log");
        let mut peer = OwnedChild::new(
            Command::new("python3")
                .args(["-c", FROZEN_OLD_PEER])
                .arg(&f.socket)
                .arg(&log)
                .arg(mode)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        eventually(|| f.socket.exists());
        let project = f.root.join("old_bun");
        fs::create_dir(&project).unwrap();
        let marker = project.join("invocations");
        fs::write(project.join("once.test.ts"), "import {test} from 'bun:test'; import {appendFileSync} from 'node:fs'; test('once', () => appendFileSync(process.env.TEST_MARKER!, 'once\\n'));\n").unwrap();
        let output = Command::new(binary("arcctl"))
            .args(["exec", "--", "bun", "test"])
            .current_dir(&project)
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env("TEST_MARKER", &marker)
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
        let requests: Vec<serde_json::Value> = fs::read_to_string(log)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(requests[0]["operation"], "governor.capabilities");
        assert_eq!(requests[1]["operation"], "traffic.bun.capabilities");
        let registrations: Vec<_> = requests
            .iter()
            .filter(|r| r["operation"] == "job.start" && r["payload"]["action"] == "REGISTER")
            .collect();
        assert_eq!(registrations.len(), 1);
        assert_eq!(
            registrations[0]["payload"]["classification"]["reason"],
            "UNRESOLVED_COMMAND"
        );
        assert_eq!(
            requests
                .iter()
                .filter(|r| r["operation"] == "traffic.bun.job")
                .count(),
            usize::from(mode == "replace")
        );
        assert!(!fs::read_dir(&f.root).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".traffic-witness-")));
        peer.child.kill().unwrap();
        peer.child.wait().unwrap();
    }
}

#[test]
fn old_peer_replaces_probed_daemon_before_bun_enqueue() {
    use std::os::unix::net::UnixListener;

    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let front = f.root.join("probe.sock");
    let listener = UnixListener::bind(&front).unwrap();
    listener.set_nonblocking(true).unwrap();
    let project = f.root.join("replacement_bun");
    fs::create_dir(&project).unwrap();
    fs::write(
        project.join("native.test.ts"),
        "import {test} from 'bun:test'; import {appendFileSync} from 'node:fs'; test('once', () => appendFileSync(process.env.TEST_MARKER!, 'once\\n'));\n",
    )
    .unwrap();
    let mut cli = OwnedChild::new(
        Command::new(binary("arcctl"))
            .args(["exec", "--", "bun", "test"])
            .current_dir(&project)
            .env(arc_core::ARC_SOCKET_PATH, &front)
            .env("TEST_MARKER", project.join("marker"))
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(File::create(project.join("stderr")).unwrap()))
            .spawn()
            .unwrap(),
    );
    let (mut downstream, request) = policy_compat::next_legacy(&listener);
    assert_eq!(
        request.operation,
        arc_ipc::traffic::OPERATION_BUN_CAPABILITIES
    );
    assert!(request.payload.is_null());
    // Only the classification-free capability probe is proxied. The new
    // daemon itself answers it; no enqueue or lineage request is proxied.
    let response = arc_ipc::exchange(&mut f.connect(), &request).unwrap();
    assert_eq!(
        response.result,
        Some(serde_json::json!({"capability":"traffic.bun.v1"}))
    );
    daemon.stop(libc::SIGTERM);
    drop(listener);
    fs::remove_file(&front).unwrap();
    let peer_log = f.root.join("pre-enqueue-old-peer-log");
    let mut old = OwnedChild::new(
        Command::new("python3")
            .args(["-c", FROZEN_OLD_PEER])
            .arg(&front)
            .arg(&peer_log)
            .arg("old")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    eventually(|| front.exists());
    arc_ipc::write_response(&mut downstream, &response).unwrap();
    drop(downstream);
    eventually(|| cli.child.try_wait().unwrap().is_some());
    assert!(cli.child.wait().unwrap().success());
    assert_eq!(
        fs::read_to_string(project.join("marker")).unwrap(),
        "once\n"
    );
    let requests: Vec<serde_json::Value> = fs::read_to_string(&peer_log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(requests[0]["operation"], "traffic.bun.job");
    assert_eq!(requests[0]["payload"]["action"], "ENQUEUE");
    assert_eq!(
        requests[0]["payload"]["classification"]["reason"],
        "BUN_TEST"
    );
    let registrations: Vec<_> = requests
        .iter()
        .filter(|r| r["operation"] == "job.start" && r["payload"]["action"] == "REGISTER")
        .collect();
    assert_eq!(registrations.len(), 1);
    assert_eq!(
        registrations[0]["payload"]["classification"]["reason"],
        "UNRESOLVED_COMMAND"
    );
    assert!(!fs::read_dir(&f.root).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".traffic-witness-")));
    old.child.kill().unwrap();
    old.child.wait().unwrap();
    fs::remove_file(front).unwrap();
    f.report();
}
