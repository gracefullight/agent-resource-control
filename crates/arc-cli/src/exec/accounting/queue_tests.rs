//! Exercise the actual admission, spawn, wait and report path with one clock.
#[allow(dead_code)]
#[path = "../../../tests/job_accounting/support.rs"]
mod support;

use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{net::UnixStream, process::CommandExt},
    },
    process::{Command, ExitCode, Output, Stdio},
    time::{Duration, Instant},
};
use support::{Fixture, PeerMode};

const HELPER: &str = "exec::accounting::queue_tests::clocked_cli_helper";
const PROFILE_HELPER: &str = "exec::accounting::queue_tests::profile_destination_probe";
const HELLO: &[u8] = b"arc-accounting-test-clock-v1";

// The ordinary test runner may inherit arbitrary variables and descriptors.
// Only consume a pre-sent handshake on the test-owned socket; an ambient clock
// variable by itself must never cause this helper to launch a command.
fn accept_test_handshake() -> bool {
    let mut bytes = [0u8; HELLO.len()];
    let n = unsafe {
        libc::recv(
            9,
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            libc::MSG_PEEK | libc::MSG_DONTWAIT,
        )
    };
    if n != bytes.len() as isize || bytes != HELLO {
        return false;
    }
    let mut received = [0u8; HELLO.len()];
    // SAFETY: fd 9 is borrowed; the native child uses it after this helper.
    let mut socket = unsafe { std::mem::ManuallyDrop::new(UnixStream::from_raw_fd(9)) };
    socket.read_exact(&mut received).is_ok() && received == HELLO
}

#[test]
fn clocked_cli_helper() {
    let Some(path) = std::env::var_os("ARC_TEST_ACCOUNTING_CLOCK") else {
        return;
    };
    if !accept_test_handshake() {
        return;
    }
    let origin = Instant::now();
    let clock_reads = std::cell::Cell::new(0);
    let clock = || {
        clock_reads.set(clock_reads.get() + 1);
        if clock_reads.get() == 2 {
            if let Some(delivery_path) = std::env::var_os("ARC_TEST_ADVANCE_AT_OBSERVATION") {
                let pending = std::path::Path::new(&delivery_path).with_extension("pending");
                fs::write(&pending, "501").unwrap();
                fs::rename(pending, delivery_path).unwrap();
            }
        }
        let seconds: u64 = fs::read_to_string(&path).unwrap().parse().unwrap();
        origin + Duration::from_secs(seconds)
    };
    let argv = vec!["cargo".into(), "build".into()];
    let result = if let Some(path) = std::env::var_os("ARC_TEST_DELIVERY_CLOCK") {
        let delivery = || {
            let millis: u64 = fs::read_to_string(&path).unwrap().parse().unwrap();
            origin + Duration::from_millis(millis)
        };
        super::super::run_with_clocks(argv, false, &clock, &delivery)
    } else {
        super::super::run_with_environment(argv, false, &clock)
    };
    assert_eq!(result, ExitCode::from(17));
}

struct Helper(crate::hooks::fixture::OwnedChild);
impl Drop for Helper {
    fn drop(&mut self) {
        // Let ARC cancel admission or forward termination and reap its own
        // native child before the helper exits, including on test unwind.
        self.0.terminate_gracefully();
    }
}

#[test]
fn profile_destination_probe() {
    println!(
        "ARC_TEST_PROFILE_DESTINATION={:?}",
        std::env::var_os("LLVM_PROFILE_FILE")
    );
}

fn isolated_helper_command(test: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .env("ARC_TEST_ACCOUNTING_CLOCK", "/nonexistent/ambient-clock")
        .args(["--exact", test, "--nocapture"]);
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    command
}

fn isolated_helper_output(test: &str) -> Output {
    let mut child = isolated_helper_command(test)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::yield_now(),
            outcome => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("isolated helper {test} did not exit: {outcome:?}");
            }
        }
    }
    child.wait_with_output().unwrap()
}

#[test]
fn ambient_clock_variable_alone_cannot_activate_the_helper() {
    let output = isolated_helper_output(HELPER);
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));

    let probe = isolated_helper_output(PROFILE_HELPER);
    assert!(probe.status.success(), "{probe:?}");
    let expected = format!(
        "ARC_TEST_PROFILE_DESTINATION={:?}",
        std::env::var_os("LLVM_PROFILE_FILE")
    );
    assert!(String::from_utf8_lossy(&probe.stdout).contains(&expected));
}

#[test]
fn production_admission_to_report_excludes_controlled_queue_time() {
    for (queue_seconds, inherited, expire, advance_at_observation) in [
        (0, false, false, false),
        (3600, false, false, false),
        (3600, true, false, false),
        (3600, false, true, false),
        (3600, false, true, true),
    ] {
        let mode = if expire && !advance_at_observation {
            PeerMode::QueuedExpireReport
        } else {
            PeerMode::Queued
        };
        let fixture = Fixture::new_with_tool(mode, "/bin/sh");
        let clock_path = fixture.root.join("clock");
        fs::write(&clock_path, "0").unwrap();
        let delivery_clock = fixture.root.join("delivery-clock");
        fs::write(&delivery_clock, "0").unwrap();
        // /bin/sh receives "build" as its script and communicates on the
        // inherited socket; no requested build or sleep is executed here.
        fs::write(fixture.root.join("build"), b"printf 'start\\n' >> marker\nprintf 'ready:%s\\n' \"${ARC_TRAFFIC_LINEAGE-unset}\" >&9\nIFS= read -r answer <&9 || exit 55\n[ \"$answer\" = go ] || exit 56\nexit 17\n").unwrap();
        let (mut controller, child_end) = UnixStream::pair().unwrap();
        controller
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        controller.write_all(HELLO).unwrap();
        let fd = child_end.as_raw_fd();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .env_clear()
            .args(["--exact", HELPER])
            .current_dir(&fixture.root)
            .env("ARC_TEST_ACCOUNTING_CLOCK", &clock_path)
            .env(arc_core::ARC_SOCKET_PATH, &fixture.socket)
            .env("PATH", &fixture.root)
            .env("HOME", &fixture.root)
            .env("TMPDIR", &fixture.root)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        if inherited {
            command.env("ARC_TRAFFIC_LINEAGE", "f".repeat(64));
        }
        if expire {
            command.env("ARC_TEST_DELIVERY_CLOCK", &delivery_clock);
        }
        if advance_at_observation {
            command.env("ARC_TEST_ADVANCE_AT_OBSERVATION", &delivery_clock);
        }
        // SAFETY: only native descriptor and signal syscalls run after fork.
        // The socket belongs to this helper and its native child.
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(fd, 9) < 0 || libc::fcntl(9, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let mut action: libc::sigaction = std::mem::zeroed();
                action.sa_sigaction = libc::SIG_DFL;
                let mut unblock: libc::sigset_t = std::mem::zeroed();
                if libc::sigemptyset(&mut action.sa_mask) != 0
                    || libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut()) != 0
                    || libc::sigemptyset(&mut unblock) != 0
                    || libc::sigaddset(&mut unblock, libc::SIGTERM) != 0
                    || libc::sigprocmask(libc::SIG_UNBLOCK, &unblock, std::ptr::null_mut()) != 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut helper = Helper(crate::hooks::fixture::OwnedChild::spawn(
            &mut command,
            "clocked accounting helper",
        ));
        drop(child_end);
        fixture.wait_for_queued_poll();
        assert!(helper.0.child_mut().try_wait().unwrap().is_none());
        set_clock(&clock_path, queue_seconds);
        fixture.release_queued();
        let mut ready = String::new();
        BufReader::new(&controller).read_line(&mut ready).unwrap();
        assert!(ready.starts_with("ready:"));
        if inherited {
            assert_eq!(ready, format!("ready:{}\n", "f".repeat(64)));
        }
        fixture.wait_for_attachment();
        assert!(helper.0.child_mut().try_wait().unwrap().is_none());
        set_clock(&clock_path, queue_seconds + 7);
        controller.write_all(b"go\n").unwrap();
        assert!(helper.0.wait().success());
        assert_eq!(
            fs::read_to_string(fixture.root.join("marker"))
                .unwrap()
                .lines()
                .count(),
            1
        );
        let state = fixture.state.lock().unwrap();
        if expire {
            assert!(
                state.report_requests.is_empty(),
                "expired terminal window sent a report"
            );
            assert!(state.reports.is_empty());
            continue;
        }
        assert_eq!(state.reports.len(), 1);
        let report = state.reports.values().next().unwrap();
        assert_eq!(
            report.run_elapsed.seconds,
            Some(7.0),
            "queue={queue_seconds}"
        );
        assert_eq!(
            report.outcome,
            arc_core::job_meter::TerminalOutcome::Exited { code: 17 }
        );
    }
}

fn set_clock(path: &std::path::Path, seconds: u64) {
    let temporary = path.with_extension("pending");
    fs::write(&temporary, seconds.to_string()).unwrap();
    fs::rename(temporary, path).unwrap();
}
