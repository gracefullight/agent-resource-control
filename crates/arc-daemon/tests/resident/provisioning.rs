use super::*;

#[test]
fn permissive_directories_and_their_contents_survive_daemon_and_cli_rejection() {
    let _serial = support::serial();
    for permissions in [0o755, 0o770] {
        let mut fixture = Fixture::new();
        let directory = fixture.directory.join("arc");
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(permissions)).unwrap();
        fs::write(directory.join("sentinel"), b"operator-owned bytes\0\xff").unwrap();
        fixture.socket = directory.join("arc.sock");
        assert_preserved_rejection(&fixture, false, &format!("permissions {permissions:04o}"));
    }
}

#[test]
fn symlink_targets_and_dangling_links_survive_daemon_and_cli_rejection() {
    let _serial = support::serial();
    for permissions in [0o700, 0o755, 0o770] {
        for intermediate in [false, true] {
            let mut fixture = Fixture::new();
            let target = fixture.directory.join("target");
            fs::create_dir(&target).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(permissions)).unwrap();
            fs::write(target.join("sentinel"), b"symlink target stays intact").unwrap();
            let link = fixture.directory.join("arc");
            symlink(&target, &link).unwrap();
            fixture.socket = if intermediate {
                link.join("missing/arc.sock")
            } else {
                link.join("arc.sock")
            };
            assert_preserved_rejection(&fixture, false, "symlink");
            if !intermediate {
                fixture.socket = fixture.directory.join("new/../arc/arc.sock");
                assert_preserved_rejection(&fixture, false, "symlink");
                assert!(!fixture.directory.join("new").exists());
            }
        }
    }
    let mut fixture = Fixture::new();
    let target = fixture.directory.join("absent");
    let link = fixture.directory.join("arc");
    symlink(&target, &link).unwrap();
    fixture.socket = link.join("arc.sock");
    assert_preserved_rejection(&fixture, false, "symlink");
    assert!(!target.exists(), "dangling target was created");
}

#[test]
fn nondirectory_parent_is_rejected_before_lock_or_database_initialization() {
    let _serial = support::serial();
    let mut fixture = Fixture::new();
    let file = fixture.directory.join("file");
    fs::write(&file, b"not a directory").unwrap();
    fixture.socket = file.join("arc.sock");
    assert_preserved_rejection(&fixture, false, "must be a directory");
}

#[test]
fn private_existing_and_new_nested_directories_support_override_startup() {
    let _serial = support::serial();
    for nested in [false, true] {
        for existing in [false, true] {
            for mask in [0o000, 0o022, 0o077] {
                let mut fixture = Fixture::new();
                let first = fixture.directory.join("arc");
                let parent = if nested {
                    first.join("nested")
                } else {
                    first.clone()
                };
                let sentinel = parent.join("sentinel");
                let before = if existing {
                    fs::DirBuilder::new()
                        .recursive(true)
                        .mode(0o700)
                        .create(&parent)
                        .unwrap();
                    fs::write(&sentinel, b"existing private content").unwrap();
                    let metadata = fs::metadata(&parent).unwrap();
                    Some((
                        metadata.dev(),
                        metadata.ino(),
                        metadata.uid(),
                        metadata.gid(),
                    ))
                } else {
                    None
                };
                fixture.socket = parent.join("arc.sock");
                let mut daemon = fixture.spawn_configured(false, Some(mask));
                wait_until_ready(&mut daemon, &fixture.socket);
                assert_eq!(mode(&first), 0o700);
                assert_eq!(mode(&parent), 0o700);
                assert_eq!(mode(&fixture.socket), 0o600);
                if let Some(before) = before {
                    let metadata = fs::metadata(&parent).unwrap();
                    assert_eq!(
                        before,
                        (
                            metadata.dev(),
                            metadata.ino(),
                            metadata.uid(),
                            metadata.gid()
                        )
                    );
                    assert_eq!(fs::read(&sentinel).unwrap(), b"existing private content");
                }
                for operation in ["status", "doctor"] {
                    let output = cli_configured(&fixture, operation, false);
                    assert!(output.status.success(), "{output:?}");
                }
                let mut duplicate = fixture.spawn_configured(false, Some(mask));
                assert!(!wait_for_exit(&mut duplicate).success());
                let mut stderr = String::new();
                duplicate
                    .stderr
                    .as_mut()
                    .unwrap()
                    .read_to_string(&mut stderr)
                    .unwrap();
                assert!(stderr.contains("socket already served"), "{stderr}");
                assert_hello(&fixture.socket);
                // SAFETY: this child is owned by the fixture and still running.
                assert_eq!(
                    unsafe { libc::kill(daemon.id() as libc::pid_t, libc::SIGTERM) },
                    0
                );
                assert!(wait_for_exit(&mut daemon).success());
                assert!(!fixture.socket.exists());
                assert_eq!(mode(&parent), 0o700);
            }
        }
    }
}

#[test]
fn overlong_socket_paths_and_existing_socket_entries_reject_without_mutation() {
    let _serial = support::serial();
    let mut fixture = Fixture::new();
    fixture.database = fixture.directory.join("db/arc.sqlite3");
    fs::write(fixture.directory.join("sentinel"), b"protected").unwrap();
    let anchor = arc_core::instance_anchor().unwrap();
    let anchor_before = fs::read(&anchor).ok();
    fixture.socket = fixture.directory.join("new").join("s".repeat(104));
    assert_preserved_rejection(&fixture, false, "maximum");
    assert_eq!(
        anchor_before,
        fs::read(&anchor).ok(),
        "overlong rejection wrote admission contents"
    );
    fixture.socket = fixture.directory.join("s".repeat(104));
    fs::write(&fixture.socket, b"overlong existing entry stays intact").unwrap();
    assert_preserved_rejection(&fixture, false, "maximum");
    assert_eq!(anchor_before, fs::read(&anchor).ok());
    let target = fixture.directory.join("target");
    fs::write(&target, b"socket target").unwrap();
    fixture.socket = fixture.directory.join("entry");
    symlink(&target, &fixture.socket).unwrap();
    assert_preserved_rejection(&fixture, false, "symlink");
    fs::remove_file(&fixture.socket).unwrap();
    fs::write(&fixture.socket, b"existing regular file").unwrap();
    assert_preserved_rejection(&fixture, false, "Unix socket");
}
