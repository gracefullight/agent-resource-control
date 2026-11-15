//! Induced pre-readiness unwinds exercise real native and route fixture trees.
use super::{native_environment::Project, native_routes::STUB, owned_tree::OwnedTree, *};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::{cell::Cell, io, path::Path};

fn expect_induced_panic(result: std::thread::Result<()>, expected: &str) {
    let payload = result.expect_err("the fixture must unwind before readiness");
    let message = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str));
    assert_eq!(message, Some(expected));
}

fn gone(identities: &[ProcessIdentity]) {
    assert!(identities.len() >= 2, "the fixture must contain a child");
    for identity in identities {
        assert!(
            matches!(
                arc_core::process_identity::probe(identity),
                IdentityStatus::Missing | IdentityStatus::Mismatch
            ),
            "owned process survived early unwind: {identity:?}"
        );
    }
}

#[test]
fn native_environment_early_unwind_cleans_direct_and_nested_go_trees() {
    let _serial = serialization::serial();
    let mut daemon = Daemon::new();
    let project = Project::new(&daemon.fixture.root, daemon.fixture.database.clone());
    for (managed, selector) in [(false, "^TestChild$"), (true, "^TestParent$")] {
        project.reset();
        let mut command = project.command(&daemon.fixture.socket, managed, selector, None);
        command.env("ARC_TEST_HOLD_READY", "1");
        let mut identities = Vec::new();
        let unwound = catch_unwind(AssertUnwindSafe(|| {
            let tree = OwnedTree::spawn(command, &daemon.fixture.root);
            eventually(|| project.root.join("early").exists());
            assert!(!project.root.join("ready").exists());
            identities = tree.snapshot().unwrap();
            for pid in fs::read_to_string(project.root.join("body-pids"))
                .unwrap()
                .lines()
            {
                assert!(identities
                    .iter()
                    .any(|item| item.pid == pid.parse::<u32>().unwrap()));
            }
            assert_eq!(project.records().len(), if managed { 2 } else { 1 });
            if managed {
                assert!(
                    identities.len() >= 6,
                    "nested ARC and Go processes: {identities:?}"
                );
                let mut rows = Vec::new();
                eventually(|| {
                    rows = daemon
                        .rows("SELECT child_pid FROM job_lifecycle WHERE child_pid IS NOT NULL");
                    rows.len() == 2
                });
                for row in rows {
                    let pid = row["child_pid"].as_i64().unwrap() as i32;
                    assert_eq!(unsafe { libc::getpgid(pid) }, pid);
                    assert_ne!(pid as u32, tree.root_id());
                    assert!(identities.iter().any(|item| item.pid == pid as u32));
                }
            }
            panic!("induced failure before native readiness");
        }));
        expect_induced_panic(unwound, "induced failure before native readiness");
        gone(&identities);
        assert!(!daemon
            .fixture
            .root
            .join("owned-cleanup-incomplete")
            .exists());
    }
    daemon.stop();
}

#[test]
fn native_environment_early_unwind_cleans_direct_and_managed_route_trees() {
    let _serial = serialization::serial();
    let mut daemon = Daemon::new();
    let root = &daemon.fixture.root;
    let executable = root.join("go");
    fs::write(&executable, STUB).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    for managed in [false, true] {
        let early = root.join("route-early");
        let starts = root.join("route-starts");
        let release = root.join("route-release");
        let _ = fs::remove_file(&early);
        let mut command = if managed {
            daemon.cli()
        } else {
            Command::new(&executable)
        };
        if managed {
            command.args(["exec", "--"]).arg(&executable);
        }
        command
            .arg("test")
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("ARC_ROUTE_HOLD_READY", "1")
            .env("ARC_ROUTE_EARLY", &early)
            .env("ARC_ROUTE_STARTS", &starts)
            .env("ARC_ROUTE_RELEASE", &release)
            .env(super::owned_tree::REGISTRY_ENV, root.join("route-owners"));
        let mut identities = Vec::new();
        let unwound = catch_unwind(AssertUnwindSafe(|| {
            let tree = OwnedTree::spawn(command, root);
            eventually(|| {
                let Ok(pid) = fs::read_to_string(&early) else {
                    return false;
                };
                if pid.trim().parse::<u32>().is_err() {
                    return false;
                }
                identities = tree.snapshot().unwrap();
                identities.len() >= if managed { 3 } else { 2 }
            });
            assert!(!starts.exists());
            let target_pid: u32 = fs::read_to_string(&early).unwrap().trim().parse().unwrap();
            assert!(identities.iter().any(|item| item.pid == target_pid));
            assert_eq!(
                unsafe { libc::getpgid(target_pid as i32) },
                target_pid as i32
            );
            if managed {
                assert_ne!(target_pid, tree.root_id());
            }
            panic!("induced failure before route readiness");
        }));
        expect_induced_panic(unwound, "induced failure before route readiness");
        gone(&identities);
        assert!(!root.join("owned-cleanup-incomplete").exists());
    }
    daemon.stop();
}

// The child deliberately leaves the wrapper's process group and keeps its
// output pipes open. No daemon, SwiftPM or project code is involved.
fn held_command(root: &Path) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            r#"/usr/bin/perl -MPOSIX -e 'POSIX::setpgid(0, 0) or die "setpgid: $!"; open(my $f, ">", $ARGV[0]) or die $!; print $f "$$\n"; close($f); my $r=fork(); defined($r) or die $!; if ($r == 0) { open(STDOUT, ">", "/dev/null"); open(STDERR, ">", "/dev/null"); $ENV{ARC_TEST_OWNER_HELPER}=1; exec($ENV{ARC_TEST_OWNER_REGISTER}, "--exact", "native_recovery::owned_tree_registration_tests::owned_tree_registration_helper", "--nocapture"); exit 125 } waitpid($r, 0); die "registration failed" if $?; sleep 300' "$1" & wait"#,
            "owned-fixture",
        ])
        .arg(root.join("held-child"))
        .env(super::owned_tree::REGISTRY_ENV, root.join("held-owners"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

#[test]
fn owned_tree_cleans_registered_child_reparented_before_any_ancestry_scan() {
    let _serial = serialization::serial();
    let fixture = native::Fixture::with_binary(Default::default());
    let registry = fixture.root.join("orphan-owners");
    let marker = fixture.root.join("orphan-pid");
    let mut command = Command::new("/usr/bin/perl");
    command
        .args([
            "-MPOSIX",
            "-e",
            r#"my $pid = fork(); defined($pid) or die $!; my $d = $ENV{ARC_TEST_OWNER_REGISTRY}; if ($pid == 0) { POSIX::setpgid(0, 0) or die $!; close(STDOUT); close(STDERR); open(my $f, ">", $ARGV[0]) or die $!; print $f "$$\n"; close($f); my $r=fork(); defined($r) or die $!; if ($r == 0) { open(STDOUT, ">", "/dev/null"); open(STDERR, ">", "/dev/null"); $ENV{ARC_TEST_OWNER_HELPER}=1; exec($ENV{ARC_TEST_OWNER_REGISTER}, "--exact", "native_recovery::owned_tree_registration_tests::owned_tree_registration_helper", "--nocapture"); exit 125 } waitpid($r, 0); exit 125 if $?; sleep 300; exit 0 } for (1..30000) { my @acks=glob("$d/$pid.*.ack"); exit 0 if @acks; select(undef, undef, undef, .001) } exit 125"#,
        ])
        .arg(&marker)
        .env(super::owned_tree::REGISTRY_ENV, &registry)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Disable ancestry enumeration entirely: the registration handshake is
    // the only possible source of the orphan's identity in this test.
    let mut tree =
        OwnedTree::spawn_with_mode(command, &fixture.root, ProcessIdentity::capture, false);
    let root = ProcessIdentity::capture(tree.root_id()).unwrap();
    let mut child = None;
    eventually(|| {
        child = fs::read_to_string(&marker)
            .ok()
            .and_then(|text| text.trim().parse::<u32>().ok())
            .and_then(|pid| ProcessIdentity::capture(pid).ok());
        child.is_some()
    });
    let child = child.unwrap();
    eventually(|| tree.has_exited().unwrap());
    assert_eq!(
        arc_core::process_identity::probe(&root),
        IdentityStatus::VerifiedMatch
    );
    eventually(|| {
        super::owned_tree::info(child.pid).is_some_and(|process| process.pbi_ppid != root.pid)
    });
    assert_eq!(unsafe { libc::getpgid(child.pid as i32) }, child.pid as i32);
    assert!(tree.recorded().contains(&child));
    let output = tree.wait_with_output_for(Duration::from_secs(5)).unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    gone(&[root, child]);
    assert!(!registry.exists());
    assert!(!fixture.root.join("owned-cleanup-incomplete").exists());
}

#[test]
fn owned_tree_cleans_unregistered_session_member_after_early_reparenting() {
    let _serial = serialization::serial();
    let fixture = native::Fixture::with_binary(Default::default());
    let marker = fixture.root.join("unregistered-pid");
    let mut command = Command::new("/usr/bin/perl");
    command
        .args([
            "-MPOSIX",
            "-e",
            r#"my $pid = fork(); defined($pid) or die $!; if ($pid == 0) { POSIX::setpgid(0, 0) or die $!; close(STDOUT); close(STDERR); open(my $f, ">", $ARGV[0]) or die $!; print $f "$$\n"; close($f); sleep 300; exit 0 } for (1..30000) { exit 0 if -s $ARGV[0]; select(undef, undef, undef, .001) } exit 125"#,
        ])
        .arg(&marker)
        .env(super::owned_tree::REGISTRY_ENV, fixture.root.join("session-owners"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut tree =
        OwnedTree::spawn_with_mode(command, &fixture.root, ProcessIdentity::capture, false);
    let root = ProcessIdentity::capture(tree.root_id()).unwrap();
    let mut child = None;
    eventually(|| {
        child = fs::read_to_string(&marker)
            .ok()
            .and_then(|text| text.trim().parse::<u32>().ok())
            .and_then(|pid| ProcessIdentity::capture(pid).ok());
        child.is_some()
    });
    let child = child.unwrap();
    eventually(|| tree.has_exited().unwrap());
    assert_eq!(
        arc_core::process_identity::probe(&root),
        IdentityStatus::VerifiedMatch
    );
    eventually(|| {
        super::owned_tree::info(child.pid).is_some_and(|process| process.pbi_ppid != root.pid)
    });
    assert_eq!(unsafe { libc::getpgid(child.pid as i32) }, child.pid as i32);
    assert_eq!(unsafe { libc::getsid(child.pid as i32) }, root.pid as i32);
    assert!(
        !tree.recorded().contains(&child),
        "no ancestry observation or registration"
    );
    let output = tree.wait_with_output_for(Duration::from_secs(5)).unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    gone(&[root, child]);
    assert!(!fixture.root.join("owned-cleanup-incomplete").exists());
}

fn held_identity(root: &Path) -> ProcessIdentity {
    let mut child = None;
    eventually(|| {
        child = fs::read_to_string(root.join("held-child"))
            .ok()
            .and_then(|value| value.trim().parse::<u32>().ok())
            .and_then(|pid| ProcessIdentity::capture(pid).ok());
        child.is_some()
    });
    let child = child.unwrap();
    assert_eq!(unsafe { libc::getpgid(child.pid as i32) }, child.pid as i32);
    child
}

#[test]
fn owned_tree_retains_and_cleans_child_after_wrapper_exits_and_output_times_out() {
    let _serial = serialization::serial();
    let fixture = native::Fixture::with_binary(Default::default());
    let mut tree = OwnedTree::spawn(held_command(&fixture.root), &fixture.root);
    let child = held_identity(&fixture.root);
    // Wait for the background observer itself; do not populate ownership via
    // the optional synchronous snapshot before terminating the wrapper.
    eventually(|| tree.recorded().contains(&child));
    let root = ProcessIdentity::capture(tree.root_id()).unwrap();
    tree.kill_root().unwrap();
    let error = tree
        .wait_with_output_for(Duration::from_millis(100))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    gone(&[root, child]);
    assert!(!fixture.root.join("owned-cleanup-incomplete").exists());
}

#[test]
fn owned_tree_initial_capture_failure_cleans_a_separately_grouped_descendant() {
    let _serial = serialization::serial();
    let fixture = native::Fixture::with_binary(Default::default());
    let identities = std::cell::RefCell::new(Vec::new());
    let unwound = catch_unwind(AssertUnwindSafe(|| {
        let _tree = OwnedTree::spawn_with_initial_capture(
            held_command(&fixture.root),
            &fixture.root,
            |pid| {
                let child = held_identity(&fixture.root);
                let root = ProcessIdentity::capture(pid).unwrap();
                *identities.borrow_mut() = vec![root, child];
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected root failure",
                ))
            },
        );
    }));
    let message = unwound.expect_err("identity capture must fail");
    let message = message.downcast_ref::<String>().unwrap();
    assert!(message.contains("injected root failure"), "{message}");
    assert!(message.contains("cleanup: Ok(())"), "{message}");
    gone(&identities.into_inner());
    assert!(!fixture.root.join("owned-cleanup-incomplete").exists());
}

#[test]
fn owned_tree_unwind_during_initial_capture_still_cleans_the_descendant() {
    let _serial = serialization::serial();
    let fixture = native::Fixture::with_binary(Default::default());
    let identities = std::cell::RefCell::new(Vec::new());
    let unwound = catch_unwind(AssertUnwindSafe(|| {
        let _tree = OwnedTree::spawn_with_initial_capture(
            held_command(&fixture.root),
            &fixture.root,
            |pid| {
                let child = held_identity(&fixture.root);
                let root = ProcessIdentity::capture(pid).unwrap();
                *identities.borrow_mut() = vec![root, child];
                panic!("injected capture unwind");
            },
        );
    }));
    expect_induced_panic(unwound, "injected capture unwind");
    gone(&identities.into_inner());
    assert!(!fixture.root.join("owned-cleanup-incomplete").exists());
}

#[test]
fn owned_tree_reports_an_unreadable_live_child_and_cleans_its_retained_identity() {
    let _serial = serialization::serial();
    let fixture = native::Fixture::with_binary(Default::default());
    let mut tree = OwnedTree::spawn(held_command(&fixture.root), &fixture.root);
    let child = held_identity(&fixture.root);
    eventually(|| tree.recorded().contains(&child));
    let root = ProcessIdentity::capture(tree.root_id()).unwrap();
    let capture_failed = Cell::new(false);
    let info_failed = Cell::new(false);
    let error = tree
        .cleanup_with(
            &|pid| {
                if pid == child.pid {
                    capture_failed.set(true);
                    Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "injected unreadable child",
                    ))
                } else {
                    ProcessIdentity::capture(pid)
                }
            },
            &|pid| {
                if pid == child.pid && capture_failed.get() && !info_failed.replace(true) {
                    None
                } else {
                    super::owned_tree::info(pid)
                }
            },
        )
        .expect_err("a live unreadable child is an incomplete cleanup observation");
    assert!(capture_failed.get());
    assert!(info_failed.get());
    assert!(error.to_string().contains("injected unreadable child"));
    assert!(fixture.root.join("owned-cleanup-incomplete").exists());
    gone(&[root, child]);
    // The marker is evidence above, not a leaked test artifact.
    fs::remove_file(fixture.root.join("owned-cleanup-incomplete")).unwrap();
}
