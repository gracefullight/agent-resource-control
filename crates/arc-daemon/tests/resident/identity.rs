use super::*;

#[cfg(target_os = "macos")]
#[test]
fn native_default_and_cli_ignore_unset_changed_and_unwritable_tmpdir() {
    let _serial = support::serial();
    let mut fixture = Fixture::new();
    fixture.socket = arc_core::runtime_root().unwrap().join("arc/arc.sock");
    let unwritable = fixture.directory.join("no-write");
    fs::DirBuilder::new()
        .mode(0o500)
        .create(&unwritable)
        .unwrap();
    assert!(
        fs::write(unwritable.join("probe"), b"cannot write").is_err(),
        "TMPDIR must be demonstrably unwritable"
    );
    let changed = fixture.directory.join("changed");
    fs::DirBuilder::new().mode(0o700).create(&changed).unwrap();
    for daemon_tmpdir in [None, Some(&changed), Some(&unwritable)] {
        let mut command = fixture.command(true);
        command.env_remove("TMPDIR");
        if let Some(path) = daemon_tmpdir {
            command.env("TMPDIR", path);
        }
        let mut daemon = Daemon(command.spawn().unwrap());
        wait_until_ready(&mut daemon, &fixture.socket);
        assert_private(fixture.socket.parent().unwrap(), 0o700);
        assert_private(&fixture.socket, 0o600);
        let anchor = arc_core::instance_anchor().unwrap();
        assert_private(anchor.parent().unwrap(), 0o700);
        assert_private(&anchor, 0o600);
        for client_tmpdir in [None, Some(&changed), Some(&unwritable)] {
            for operation in ["status", "doctor"] {
                let mut client = Command::new(wait_for_binary("arcctl"));
                client
                    .arg(operation)
                    .env_remove(arc_core::ARC_SOCKET_PATH)
                    .env_remove("TMPDIR");
                if let Some(path) = client_tmpdir {
                    client.env("TMPDIR", path);
                }
                let output = client.output().unwrap();
                assert!(output.status.success(), "{output:?}");
                if operation == "status" {
                    assert!(String::from_utf8_lossy(&output.stdout)
                        .contains(fixture.socket.to_str().unwrap()));
                }
            }
        }
        stop(&mut daemon);
        assert!(!fixture.socket.exists());
    }
    assert!(!changed.join("arc").exists());
    assert!(!unwritable.join("arc").exists());
    fs::set_permissions(unwritable, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn concurrent_distinct_overrides_admit_one_and_recover_after_shutdown_and_death() {
    let _serial = support::serial();
    let mut fixtures = [Fixture::new(), Fixture::new()];
    for (index, fixture) in fixtures.iter_mut().enumerate() {
        fixture.socket = fixture.directory.join(format!("ipc/{index}.sock"));
        fixture.database = fixture.directory.join("db/arc.sqlite3");
        fs::write(
            fixture.directory.join("sentinel"),
            b"preserve contender content",
        )
        .unwrap();
    }
    let before = [
        snapshot(&fixtures[0].directory),
        snapshot(&fixtures[1].directory),
    ];
    let mut children = [fixtures[0].spawn_gated(), fixtures[1].spawn_gated()];
    for child in &mut children {
        child.stdin.take().unwrap().write_all(b"go\n").unwrap();
    }
    let deadline = Instant::now() + READY_TIMEOUT;
    let loser = loop {
        let exits = [
            children[0].try_wait().unwrap(),
            children[1].try_wait().unwrap(),
        ];
        assert!(
            !(exits[0].is_some() && exits[1].is_some()),
            "both contenders exited: {exits:?}"
        );
        if let Some(index) = exits.iter().position(Option::is_some) {
            break index;
        }
        assert!(Instant::now() < deadline, "neither contender was rejected");
        thread::sleep(Duration::from_millis(10));
    };
    let winner = 1 - loser;
    assert!(!wait_for_exit(&mut children[loser]).success());
    let mut stdout = String::new();
    let mut stderr = String::new();
    children[loser]
        .stdout
        .as_mut()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    children[loser]
        .stderr
        .as_mut()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(stdout.is_empty(), "loser announced readiness: {stdout}");
    assert!(stderr.contains("only one daemon per user"), "{stderr}");
    assert_eq!(before[loser], snapshot(&fixtures[loser].directory));
    assert!(!fixtures[loser].database.parent().unwrap().exists());
    assert!(!fixtures[loser].socket.parent().unwrap().exists());
    for suffix in ["", "-wal", "-shm"] {
        assert!(!PathBuf::from(format!("{}{suffix}", fixtures[loser].database.display())).exists());
    }
    let fixture = &fixtures[winner];
    wait_until_ready(&mut children[winner], &fixture.socket);
    for operation in ["status", "doctor"] {
        assert!(arcctl(fixture, operation).status.success());
    }
    assert_private(fixture.socket.parent().unwrap(), 0o700);
    assert_private(&fixture.socket, 0o600);
    assert_private(&arc_core::instance_anchor().unwrap(), 0o600);
    stop(&mut children[winner]);
    assert!(!fixture.socket.exists());

    let mut graceful_restart = fixture.spawn();
    wait_until_ready(&mut graceful_restart, &fixture.socket);
    for operation in ["status", "doctor"] {
        assert!(arcctl(fixture, operation).status.success());
    }
    graceful_restart.kill().unwrap();
    assert!(!wait_for_exit(&mut graceful_restart).success());
    assert!(
        fixture.socket.exists(),
        "forced death must leave the stale socket for recovery"
    );
    let mut recovered = fixture.spawn();
    wait_until_ready(&mut recovered, &fixture.socket);
    for operation in ["status", "doctor"] {
        assert!(arcctl(fixture, operation).status.success());
    }
    stop(&mut recovered);
    assert!(!fixture.socket.exists());
}
