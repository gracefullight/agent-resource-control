use super::*;
use std::{
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{net::UnixStream, process::CommandExt},
    },
    panic::{catch_unwind, AssertUnwindSafe},
    process::Stdio,
    time::{Duration, Instant},
};

pub(super) fn prepare(root: &Path, id: &str) -> PathBuf {
    let directory = root.join("target/host-meter/legacy").join(id);
    fs::create_dir_all(directory.join("target/debug")).unwrap();
    for suffix in [
        "accepted.tar",
        "stdout.log",
        "stderr.log",
        "target/debug/arc-daemon",
        "target/debug/arcctl",
        "target/debug/arc-exec-fixture",
    ] {
        fs::write(directory.join(suffix), suffix.as_bytes()).unwrap();
    }
    let reference = |suffix: &str| {
        let file = directory.join(suffix);
        serde_json::json!({"path":format!("target/host-meter/legacy/{id}/{suffix}"),
                "bytes":fs::metadata(&file).unwrap().len(), "sha256":receipt::sha256(&file)})
    };
    let receipt = serde_json::json!({"acceptedHead":"efedf0f4eb9b97eed641c82f3c4766bb16f2e3de", "exitCode":0,
            "archive":reference("accepted.tar"), "stdout":reference("stdout.log"), "stderr":reference("stderr.log"),
            "binary":reference("target/debug/arc-daemon"), "binaries":{
                "arcctl":reference("target/debug/arcctl"), "fixture":reference("target/debug/arc-exec-fixture")}});
    let bytes = serde_json::to_vec(&receipt).unwrap();
    fs::write(directory.join("receipt.json"), &bytes).unwrap();
    fs::write(root.join("target/host-meter/legacy/latest.json"), bytes).unwrap();
    directory
}

pub(super) fn cleanup_root(root: &Path) {
    let lock = coordination_path(root);
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(lock).unwrap();
}

pub(super) fn wait_for_child(
    child: &mut crate::native::OwnedChild,
    description: &str,
) -> std::process::ExitStatus {
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.child.try_wait().expect("observe legacy child exit") {
            return status;
        }
        if Instant::now() >= end {
            crate::native::native_readiness::kill_and_reap(&mut child.child, false)
                .expect("terminate and reap legacy child after watchdog");
            panic!("legacy child never exited: {description}");
        }
        std::thread::yield_now();
    }
}

#[test]
fn fixture_termination_reaps_the_direct_child() {
    use std::os::unix::process::ExitStatusExt;
    let mut child = crate::native::OwnedChild::new(
        Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    crate::native::native_readiness::kill_and_reap(&mut child.child, false).unwrap();
    assert_eq!(
        child.child.try_wait().unwrap().unwrap().signal(),
        Some(libc::SIGKILL)
    );
}

#[test]
fn fixture_termination_reaps_an_owned_process_group() {
    use std::os::unix::process::ExitStatusExt;
    let mut command = Command::new("/bin/cat");
    command
        .env_clear()
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = crate::native::OwnedChild::new(command.spawn().unwrap());
    assert_eq!(
        unsafe { libc::getpgid(child.child.id() as i32) },
        child.child.id() as i32
    );
    crate::native::native_readiness::kill_and_reap(&mut child.child, true).unwrap();
    assert_eq!(
        child.child.try_wait().unwrap().unwrap().signal(),
        Some(libc::SIGKILL)
    );
}

#[test]
fn fixture_reap_accepts_a_child_that_already_exited() {
    let mut child = crate::native::OwnedChild::new(
        Command::new("/usr/bin/true")
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    assert!(wait_for_child(&mut child, "already exited fixture").success());
    crate::native::native_readiness::kill_and_reap(&mut child.child, false).unwrap();
}

#[test]
fn fixture_wait4_reaps_exit_and_reports_native_usage() {
    let mut child = crate::native::OwnedChild::new(
        Command::new("/usr/bin/true")
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let (status, usage) = crate::native::native_readiness::wait4_usage(&mut child.child).unwrap();
    assert!(libc::WIFEXITED(status));
    assert_eq!(libc::WEXITSTATUS(status), 0);
    assert!(usage.ru_utime.tv_sec >= 0 && usage.ru_stime.tv_sec >= 0);
    assert_eq!(
        child.child.try_wait().unwrap_err().raw_os_error(),
        Some(libc::ECHILD)
    );
}

#[test]
fn fixture_watched_output_preserves_streams_and_exit_status() {
    let mut child = crate::native::OwnedChild::new(
        Command::new("/bin/sh")
            .env_clear()
            .args(["-c", "printf stdout; printf stderr >&2; exit 7"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let output = child
        .wait_with_output_for(Duration::from_secs(60), false)
        .unwrap();
    assert_eq!(output.stdout, b"stdout");
    assert_eq!(output.stderr, b"stderr");
    assert_eq!(output.status.code(), Some(7));
}

#[test]
fn legacy_consumer_rejects_tampered_artifacts_and_receipt_mismatch() {
    let root = std::env::temp_dir().join(format!(
        "arc-legacy-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let directory = prepare(&root, "00000000-0000-4000-8000-000000000001");
    for suffix in [
        "accepted.tar",
        "stdout.log",
        "stderr.log",
        "target/debug/arc-daemon",
        "target/debug/arcctl",
        "target/debug/arc-exec-fixture",
    ] {
        let path = directory.join(suffix);
        let original = fs::read(&path).unwrap();
        let mut tampered = original.clone();
        tampered[0] ^= 1;
        fs::write(&path, tampered).unwrap();
        assert!(
            catch_unwind(AssertUnwindSafe(|| LegacyPreparation::acquire(&root))).is_err(),
            "tampered {suffix} was accepted"
        );
        fs::write(&path, original).unwrap();
    }
    let pointer = root.join("target/host-meter/legacy/latest.json");
    let original = fs::read(&pointer).unwrap();
    let mut mismatched = original.clone();
    mismatched.push(b' ');
    fs::write(&pointer, mismatched).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| LegacyPreparation::acquire(&root))).is_err());
    fs::write(&pointer, original).unwrap();
    let receipt_path = directory.join("receipt.json");
    let original = fs::read(&receipt_path).unwrap();
    let mut incomplete: serde_json::Value = serde_json::from_slice(&original).unwrap();
    incomplete["binaries"]
        .as_object_mut()
        .unwrap()
        .remove("fixture");
    let incomplete = serde_json::to_vec(&incomplete).unwrap();
    fs::write(&pointer, &incomplete).unwrap();
    fs::write(&receipt_path, &incomplete).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| LegacyPreparation::acquire(&root))).is_err());
    fs::write(&pointer, &original).unwrap();
    fs::write(&receipt_path, original).unwrap();
    drop(LegacyPreparation::acquire(&root));
    assert!(!directory.exists());
    cleanup_root(&root);
}

#[test]
fn legacy_preparations_retire_after_success_and_retain_failure() {
    let root = std::env::temp_dir().join(format!(
        "arc-legacy-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let first = prepare(&root, "00000000-0000-4000-8000-000000000001");
    let lease = LegacyPreparation::acquire(&root);
    assert_eq!(lease.receipt["exitCode"], 0);
    assert!(first.exists());
    drop(lease);
    assert!(!first.exists());
    assert!(!root.join("target/host-meter/legacy/latest.json").exists());

    let second = prepare(&root, "00000000-0000-4000-8000-000000000002");
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _lease = LegacyPreparation::acquire(&root);
        panic!("intentional failure retains its peer");
    }));
    assert!(result.is_err());
    assert!(second.exists());
    assert!(second.join("failed-case").exists());
    assert!(!root.join("target/host-meter/legacy/latest.json").exists());
    let third = prepare(&root, "00000000-0000-4000-8000-000000000003");
    drop(LegacyPreparation::acquire(&root));
    assert!(!third.exists());
    assert!(second.exists());
    cleanup_root(&root);
}

#[test]
fn legacy_preparation_lives_until_its_last_consumer() {
    let root = std::env::temp_dir().join(format!(
        "arc-legacy-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir_all(&root).unwrap();
    let id = "00000000-0000-4000-8000-000000000001";
    let first = LegacyPreparation::ensure_with(&root, |_| {
        prepare(&root, id);
    });
    let second = LegacyPreparation::ensure_with(&root, |_| panic!("existing peer must be reused"));
    let directory = root.join("target/host-meter/legacy").join(id);
    drop(first);
    assert!(directory.exists());
    assert!(root.join("target/host-meter/legacy/latest.json").exists());
    drop(second);
    assert!(!directory.exists());
    assert!(!root.join("target/host-meter/legacy/latest.json").exists());

    let failed = prepare(&root, "00000000-0000-4000-8000-000000000002");
    let failing = LegacyPreparation::acquire(&root);
    let survivor = LegacyPreparation::acquire(&root);
    assert!(catch_unwind(AssertUnwindSafe(move || {
        let _lease = failing;
        panic!("one consumer failed");
    }))
    .is_err());
    drop(survivor);
    assert!(failed.join("failed-case").exists());
    assert!(!root.join("target/host-meter/legacy/latest.json").exists());
    cleanup_root(&root);
}

#[test]
fn retiring_an_older_lease_preserves_a_newer_pointer() {
    let root = std::env::temp_dir().join(format!(
        "arc-legacy-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let first = prepare(&root, "00000000-0000-4000-8000-000000000001");
    let lease = LegacyPreparation::acquire(&root);
    let second = prepare(&root, "00000000-0000-4000-8000-000000000002");
    drop(lease);
    assert!(!first.exists());
    assert!(second.exists());
    drop(LegacyPreparation::acquire(&root));
    assert!(!second.exists());
    cleanup_root(&root);
}

#[test]
fn legacy_lease_child() {
    if !std::env::args().any(|arg| arg == "legacy_preparation::tests::legacy_lease_child") {
        return;
    }

    let Some(root) = std::env::var_os("ARC_LEGACY_LEASE_CHILD") else {
        return;
    };
    let lease = LegacyPreparation::acquire(Path::new(&root));
    let mut channel = unsafe { UnixStream::from_raw_fd(9) };
    channel
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    channel.write_all(b"ready").unwrap();
    let mut release = [0];
    channel
        .read_exact(&mut release)
        .expect("parent never released legacy consumer");
    drop(lease);
}

#[test]
fn legacy_preparation_waits_for_another_process_to_release_it() {
    let root = std::env::temp_dir().join(format!(
        "arc-legacy-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let directory = prepare(&root, "00000000-0000-4000-8000-000000000001");
    let parent = LegacyPreparation::acquire(&root);
    let (mut channel, peer) = UnixStream::pair().unwrap();
    channel
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    channel
        .set_write_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let fd = peer.as_raw_fd();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .env("ARC_LEGACY_LEASE_CHILD", &root)
        .args(["--exact", "legacy_preparation::tests::legacy_lease_child"])
        .stdout(Stdio::null());
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(fd, 9) < 0 || libc::fcntl(9, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = crate::native::OwnedChild::new(command.spawn().unwrap());
    drop(peer);
    let mut ready = [0; 5];
    channel
        .read_exact(&mut ready)
        .expect("legacy consumer did not become ready");
    assert_eq!(&ready, b"ready");
    drop(parent);
    assert!(directory.exists());
    channel.write_all(b"x").unwrap();
    assert!(wait_for_child(&mut child, "consumer after lease release").success());
    assert!(!directory.exists());
    assert!(!root.join("target/host-meter/legacy/latest.json").exists());
    cleanup_root(&root);
}

#[test]
fn legacy_inherited_descriptor_child() {
    if !std::env::args()
        .any(|arg| arg == "legacy_preparation::tests::legacy_inherited_descriptor_child")
    {
        return;
    }
    let Some(fd) = std::env::var_os("ARC_LEGACY_LEASE_CHANNEL") else {
        return;
    };
    let fd: i32 = fd.to_string_lossy().parse().unwrap();
    let mut channel = unsafe { UnixStream::from_raw_fd(fd) };
    channel
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    channel.write_all(b"ready").unwrap();
    let mut release = [0];
    channel
        .read_exact(&mut release)
        .expect("parent never released descriptor child");
}

#[test]
fn inherited_lease_descriptor_does_not_delay_last_consumer_cleanup() {
    let root = std::env::temp_dir().join(format!(
        "arc-legacy-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let directory = prepare(&root, "00000000-0000-4000-8000-000000000001");
    let lease = LegacyPreparation::acquire(&root);
    let lease_fd = lease.lease.as_ref().unwrap().as_raw_fd();
    let (mut channel, peer) = UnixStream::pair().unwrap();
    channel
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    channel
        .set_write_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let peer_fd = peer.as_raw_fd();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .env("ARC_LEGACY_LEASE_CHANNEL", peer_fd.to_string())
        .args([
            "--exact",
            "legacy_preparation::tests::legacy_inherited_descriptor_child",
        ])
        .stdout(Stdio::null());
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    unsafe {
        command.pre_exec(move || {
            for fd in [lease_fd, peer_fd] {
                if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let mut child = crate::native::OwnedChild::new(command.spawn().unwrap());
    drop(peer);
    let mut ready = [0; 5];
    channel
        .read_exact(&mut ready)
        .expect("inherited descriptor child did not become ready");
    assert_eq!(&ready, b"ready");
    drop(lease);
    assert!(!directory.exists());
    channel.write_all(b"x").unwrap();
    assert!(wait_for_child(&mut child, "inherited-descriptor consumer after release").success());
    cleanup_root(&root);
}
