use super::tests::{cleanup_root, prepare, wait_for_child};
use super::*;
use std::{
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{net::UnixStream, process::CommandExt},
    },
    process::Stdio,
    time::Duration,
};

#[test]
fn legacy_competing_consumer_helper() {
    if !std::env::args()
        .any(|arg| arg == "legacy_preparation::race_tests::legacy_competing_consumer_helper")
    {
        return;
    }
    let Some(root) = std::env::var_os("ARC_LEGACY_RACE_ROOT") else {
        return;
    };
    let fd: i32 = std::env::var("ARC_LEGACY_RACE_FD")
        .unwrap()
        .parse()
        .unwrap();
    let mut channel = unsafe { UnixStream::from_raw_fd(fd) };
    channel
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let root = Path::new(&root);
    if std::env::var_os("ARC_LEGACY_EXPECT_CONTENTION").is_some() {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(coordination_path(root))
            .unwrap();
        assert!(matches!(file.try_lock(), Err(fs::TryLockError::WouldBlock)));
        channel.write_all(b"C").unwrap();
    }
    let mut lease = LegacyPreparation::ensure_with(root, |_| {
        channel.write_all(b"B").unwrap();
        let mut release = [0];
        channel
            .read_exact(&mut release)
            .expect("preparation was never released");
        prepare(root, "00000000-0000-4000-8000-000000000001");
    });
    channel.write_all(b"L").unwrap();
    let mut release = [0];
    channel
        .read_exact(&mut release)
        .expect("consumer was never released");
    if std::env::var_os("ARC_LEGACY_PAUSE_RETIRE").is_some() {
        lease.retire_with(|| {
            channel.write_all(b"R").unwrap();
            channel
                .read_exact(&mut release)
                .expect("retirement was never released");
        });
    }
    drop(lease);
}

fn competing_consumer(
    root: &Path,
    expect_contention: bool,
    pause_retirement: bool,
) -> (crate::native::OwnedChild, UnixStream) {
    let (channel, peer) = UnixStream::pair().unwrap();
    channel
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    channel
        .set_write_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .env("ARC_LEGACY_RACE_ROOT", root)
        .env("ARC_LEGACY_RACE_FD", "0")
        .args([
            "--exact",
            "legacy_preparation::race_tests::legacy_competing_consumer_helper",
            "--nocapture",
        ])
        .stdin(Stdio::from(OwnedFd::from(peer)))
        .stdout(Stdio::null());
    if expect_contention {
        command.env("ARC_LEGACY_EXPECT_CONTENTION", "1");
    }
    if pause_retirement {
        command.env("ARC_LEGACY_PAUSE_RETIRE", "1");
    }
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let child = crate::native::OwnedChild::new(command.spawn().unwrap());
    (child, channel)
}

#[track_caller]
fn read_step(channel: &mut UnixStream, expected: u8) {
    let mut step = [0];
    channel.read_exact(&mut step).unwrap_or_else(|error| {
        panic!(
            "legacy consumer did not publish state {}: {error}",
            expected as char
        )
    });
    assert_eq!(step[0], expected);
}

#[test]
fn legacy_preparation_and_retirement_are_serialized_across_processes() {
    let root = std::env::temp_dir().join(format!(
        "arc-legacy-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir_all(&root).unwrap();
    let (mut first, mut first_channel) = competing_consumer(&root, false, false);
    read_step(&mut first_channel, b'B');
    let (mut second, mut second_channel) = competing_consumer(&root, true, false);
    read_step(&mut second_channel, b'C');
    first_channel.write_all(b"x").unwrap();
    read_step(&mut first_channel, b'L');
    read_step(&mut second_channel, b'L');
    let directory = root.join("target/host-meter/legacy/00000000-0000-4000-8000-000000000001");
    assert!(directory.exists());
    first_channel.write_all(b"x").unwrap();
    assert!(wait_for_child(&mut first, "first competing consumer").success());
    assert!(directory.exists());
    second_channel.write_all(b"x").unwrap();
    assert!(wait_for_child(&mut second, "last competing consumer").success());
    assert!(!directory.exists());
    assert!(!root.join("target/host-meter/legacy/latest.json").exists());
    cleanup_root(&root);
}

#[test]
fn last_retirement_finishes_before_competing_acquisition_prepares_again() {
    let root = std::env::temp_dir().join(format!(
        "arc-legacy-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir_all(&root).unwrap();
    let (mut first, mut first_channel) = competing_consumer(&root, false, true);
    read_step(&mut first_channel, b'B');
    first_channel.write_all(b"x").unwrap();
    read_step(&mut first_channel, b'L');
    first_channel.write_all(b"x").unwrap();
    read_step(&mut first_channel, b'R');
    let (mut second, mut second_channel) = competing_consumer(&root, true, false);
    read_step(&mut second_channel, b'C');
    first_channel.write_all(b"x").unwrap();
    assert!(wait_for_child(&mut first, "retiring last consumer").success());
    // The second process observed the held retirement lock, and can prepare
    // only after the retired pointer and directory have both been removed.
    read_step(&mut second_channel, b'B');
    second_channel.write_all(b"x").unwrap();
    read_step(&mut second_channel, b'L');
    second_channel.write_all(b"x").unwrap();
    assert!(wait_for_child(&mut second, "consumer after retirement").success());
    assert!(!root
        .join("target/host-meter/legacy/00000000-0000-4000-8000-000000000001")
        .exists());
    assert!(!root.join("target/host-meter/legacy/latest.json").exists());
    cleanup_root(&root);
}

#[test]
fn legacy_cross_language_lock_probe() {
    if !std::env::args()
        .any(|arg| arg == "legacy_preparation::race_tests::legacy_cross_language_lock_probe")
    {
        return;
    }
    let Some(root) = std::env::var_os("ARC_LEGACY_PROBE_ROOT") else {
        return;
    };
    let root = Path::new(&root);
    let expected = std::env::var("ARC_LEGACY_PROBE_EXPECT").unwrap();
    if expected == "inherited" {
        let bun = std::env::var_os("ARC_LEGACY_PROBE_BUN").unwrap();
        let library = std::env::var("ARC_LEGACY_PROBE_MODULE").unwrap();
        let lock = coordination_lock(root);
        let fd = lock.as_raw_fd();
        let script = format!(
            "import {{lockLegacyPreparation}} from {}; const release = lockLegacyPreparation({}); release();",
            serde_json::to_string(&library).unwrap(),
            serde_json::to_string(root.to_str().unwrap()).unwrap()
        );
        let mut command = Command::new(bun);
        command
            .env_clear()
            .env("ARC_LEGACY_COORDINATION_FD", fd.to_string())
            .args(["-e", &script])
            .stdout(Stdio::null());
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = crate::native::OwnedChild::new(command.spawn().unwrap());
        assert!(wait_for_child(&mut child, "Bun inheriting legacy coordination lock").success());
    } else {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(coordination_path(root))
            .unwrap();
        let result = file.try_lock();
        if expected == "contended" {
            assert!(matches!(result, Err(fs::TryLockError::WouldBlock)));
        } else {
            assert_eq!(expected, "available");
            result.unwrap();
        }
    }
}
