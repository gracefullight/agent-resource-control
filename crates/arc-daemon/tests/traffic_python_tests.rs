//! Required macOS acceptance: real offline Python/pytest and mixed TEST runners.
#[cfg(not(target_os = "macos"))]
#[test]
fn required_native_platform() {
    panic!("Python TEST acceptance requires macOS and installed Python/pytest/Cargo/Bun");
}
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "traffic_cli/process_cleanup.rs"]
mod cleanup;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "support/legacy_preparation.rs"]
mod legacy_preparation;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "support/native_fixture.rs"]
mod native;
#[cfg(target_os = "macos")]
#[path = "support/mod.rs"]
mod serialization;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "traffic_python_tests/mod.rs"]
mod suite;

#[cfg(target_os = "macos")]
#[test]
fn native_python_execution_semantics() {
    let _serial = serialization::serial();
    suite::semantics(suite::direct_read);
}
#[cfg(target_os = "macos")]
#[test]
fn native_python_lifecycle_and_compatibility() {
    let _serial = serialization::serial();
    suite::lifecycle(suite::direct_read);
}
#[cfg(target_os = "macos")]
#[test]
fn python_then_cargo_one_test_slot() {
    let _serial = serialization::serial();
    suite::one_test_slot(suite::direct_read, suite::Peer::Cargo, true);
}
#[cfg(target_os = "macos")]
#[test]
fn cargo_then_python_one_test_slot() {
    let _serial = serialization::serial();
    suite::one_test_slot(suite::direct_read, suite::Peer::Cargo, false);
}
#[cfg(target_os = "macos")]
#[test]
fn python_then_bun_one_test_slot() {
    let _serial = serialization::serial();
    suite::one_test_slot(suite::direct_read, suite::Peer::Bun, true);
}
#[cfg(target_os = "macos")]
#[test]
fn bun_then_python_one_test_slot() {
    let _serial = serialization::serial();
    suite::one_test_slot(suite::direct_read, suite::Peer::Bun, false);
}

#[cfg(target_os = "macos")]
#[test]
fn native_fixture_cleanup_is_owned_on_success_and_unwind() {
    use std::{
        fs,
        process::{Command, Stdio},
    };

    let binary = std::env::current_exe().unwrap();
    let success = native::Fixture::with_binary(binary.clone());
    let success_root = success.root.clone();
    let failure = native::Fixture::with_binary(binary);
    let failure_root = failure.root.clone();
    let child = Command::new("/bin/cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let identity = arc_core::process_identity::ProcessIdentity::capture(child.id()).unwrap();
    let unwind = std::panic::catch_unwind(move || {
        let _fixture = failure;
        let _child = native::OwnedChild::new(child);
        panic!("intentional fixture failure");
    });
    assert!(unwind.is_err());
    assert!(failure_root.is_dir());
    assert!(success_root.is_dir());
    assert!(matches!(
        arc_core::process_identity::probe(&identity),
        arc_core::process_identity::IdentityStatus::Missing
            | arc_core::process_identity::IdentityStatus::Mismatch
    ));
    drop(success);
    assert!(!success_root.exists());
    fs::remove_dir_all(failure_root).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn native_fixture_root_skips_a_leftover_directory_under_a_reused_pid() {
    use std::{fs, sync::atomic::AtomicU64};

    let parent = native::Fixture::with_binary(std::env::current_exe().unwrap());
    let name = |counter: u8| {
        parent
            .root
            .join(format!("b{:x}-{counter:x}", std::process::id()))
    };
    fs::create_dir(name(0)).unwrap();
    let root = native::native_root::fresh(&parent.root, &AtomicU64::new(0));
    assert_eq!(root, name(1));
    assert!(name(0).is_dir() && root.is_dir());
}

#[cfg(target_os = "macos")]
#[test]
fn native_fixture_readiness_observes_marker_and_early_exit() {
    use std::{io::Write, panic::AssertUnwindSafe, path::PathBuf, time::Duration};

    let fixture = native::Fixture::with_binary(PathBuf::from("/bin/sh"));
    let mut command = fixture.command();
    command.args(["-c", "printf 'ready\\n'; IFS= read -r release"]);
    let mut child = native::OwnedChild::new(command.spawn().unwrap());
    fixture.wait_for(&mut child, "ready");
    assert!(child.child.try_wait().unwrap().is_none());
    child
        .child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"release\n")
        .unwrap();
    assert!(child.wait_with_output().unwrap().status.success());
    drop(child);

    let failed = native::Fixture::with_binary(PathBuf::from("/usr/bin/false"));
    let mut child = native::OwnedChild::new(failed.command().spawn().unwrap());
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        failed.wait_for(&mut child, "never-published");
    }));
    assert!(result.is_err());

    // The first read precedes release; the next one must see the marker even
    // though the child has already exited by the time try_wait runs.
    let published = native::Fixture::with_binary(PathBuf::from("/bin/sh"));
    let mut command = published.command();
    command.args(["-c", "IFS= read -r release; printf 'late-marker\\n'"]);
    let mut child = native::OwnedChild::new(command.spawn().unwrap());
    assert!(native::native_readiness::observed_after_read(
        &published,
        &mut child,
        "late-marker",
        |child| {
            child
                .child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"release\n")
                .unwrap();
            assert!(child.wait_with_output().unwrap().status.success());
        }
    ));

    // A live peer that publishes nothing fails at the watchdog, rather than
    // hanging the suite; zero is injected solely to exercise that failure path.
    let stalled = native::Fixture::with_binary(PathBuf::from("/bin/cat"));
    let mut child = native::OwnedChild::new(stalled.command().spawn().unwrap());
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        native::native_readiness::wait_for_with_timeout(
            &stalled,
            &mut child,
            "never-published",
            Duration::ZERO,
        );
    }));
    assert!(result.is_err());
    assert!(child.child.try_wait().unwrap().is_none());
}
