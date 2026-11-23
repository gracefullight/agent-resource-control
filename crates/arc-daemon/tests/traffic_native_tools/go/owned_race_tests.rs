//! A native child exits after observation but before event registration.
use super::*;
use std::{cell::Cell, fs, process::Stdio};

#[test]
fn already_exited_child_does_not_wait_for_a_missed_kqueue_event() {
    let root = std::env::temp_dir().join(format!(
        "arc-go-exit-race-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&root).unwrap();
    let ready = root.join("ready");
    let release = root.join("release");
    let mut command = std::process::Command::new("/bin/sh");
    command
        .env_clear()
        .env("HOME", &root)
        .env("ARC_READY", &ready)
        .env("ARC_RELEASE", &release);
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    command
        .args([
            "-c",
            "exec 1>&- 2>&-; : > \"$ARC_READY\"; while [ ! -e \"$ARC_RELEASE\" ]; do :; done",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = GoChild::spawn(&mut command).unwrap();
    let pid = child.child.id();
    let watchdog = Instant::now() + Duration::from_secs(60);
    while !ready.is_file() {
        assert!(Instant::now() < watchdog, "fixture did not close its pipes");
        std::thread::yield_now();
    }
    let released = Cell::new(false);
    let now = || {
        if !released.replace(true) {
            fs::write(&release, b"").unwrap();
            loop {
                let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
                assert_eq!(
                    unsafe {
                        libc::waitid(
                            libc::P_PID,
                            pid,
                            &mut info,
                            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                        )
                    },
                    0
                );
                if info.si_pid != 0 {
                    break;
                }
                assert!(Instant::now() < watchdog, "fixture did not exit");
                std::thread::yield_now();
            }
        }
        Instant::now()
    };
    let output = child
        .wait_with_clock(watchdog, now, |_, _| {
            panic!("waiting for an exit event after the native child already exited")
        })
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn silent_exit_is_rechecked_before_the_child_watchdog() {
    let root = std::env::temp_dir().join(format!(
        "arc-go-recheck-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&root).unwrap();
    let ready = root.join("ready");
    let release = root.join("release");
    let mut command = std::process::Command::new("/bin/sh");
    command
        .env_clear()
        .env("HOME", &root)
        .env("ARC_READY", &ready)
        .env("ARC_RELEASE", &release);
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    command
        .args([
            "-c",
            "exec 1>&- 2>&-; : > \"$ARC_READY\"; while [ ! -e \"$ARC_RELEASE\" ]; do :; done",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = GoChild::spawn(&mut command).unwrap();
    let pid = child.child.id();
    let watchdog = Instant::now() + Duration::from_secs(60);
    while !ready.is_file() {
        assert!(
            Instant::now() < watchdog,
            "fixture did not reach its barrier"
        );
        std::thread::yield_now();
    }
    let mut observed = false;
    let output = child
        .wait_with_clock(watchdog, Instant::now, |_, next_check| {
            assert!(
                next_check < watchdog,
                "event wait must leave time to recheck child state"
            );
            fs::write(&release, b"").unwrap();
            loop {
                let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
                assert_eq!(
                    unsafe {
                        libc::waitid(
                            libc::P_PID,
                            pid,
                            &mut info,
                            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                        )
                    },
                    0
                );
                if info.si_pid != 0 {
                    observed = true;
                    return Ok(());
                }
                assert!(Instant::now() < watchdog, "fixture did not exit");
                std::thread::yield_now();
            }
        })
        .unwrap();
    assert!(observed, "the fixture must exit before the state recheck");
    assert!(output.status.success());
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    fs::remove_dir_all(root).unwrap();
}
