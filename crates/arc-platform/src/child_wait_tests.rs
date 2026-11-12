use super::*;
use std::{process::Command, time::Instant};

#[test]
fn timeval_units_are_core_seconds_with_zero_distinct_from_invalid() {
    let zero = CpuTimeval {
        seconds: 0,
        microseconds: 0,
    };
    let user = CpuTimeval {
        seconds: 1,
        microseconds: 250_000,
    };
    let system = CpuTimeval {
        seconds: 2,
        microseconds: 500_000,
    };
    assert_eq!(
        TerminalUsage::from_timevals(zero, zero)
            .unwrap()
            .total_core_seconds,
        0.0
    );
    let usage = TerminalUsage::from_timevals(user, system).unwrap();
    assert_eq!(usage.raw_user, user);
    assert_eq!(usage.user_core_seconds, 1.25);
    assert_eq!(usage.system_core_seconds, 2.5);
    assert_eq!(usage.total_core_seconds, 3.75);
    for invalid in [
        CpuTimeval {
            seconds: -1,
            microseconds: 0,
        },
        CpuTimeval {
            seconds: 0,
            microseconds: -1,
        },
        CpuTimeval {
            seconds: 0,
            microseconds: 1_000_000,
        },
        CpuTimeval {
            seconds: i64::MAX,
            microseconds: 0,
        },
    ] {
        assert_eq!(
            TerminalUsage::from_timevals(invalid, zero),
            Err(UsageError::InvalidTimeval)
        );
        assert!(TerminalUsage::from_timevals(zero, invalid).is_err());
    }
    let huge = CpuTimeval {
        seconds: 9_007_199_254_740_990,
        microseconds: 0,
    };
    assert!(TerminalUsage::from_timevals(huge, huge).is_err());
}

#[test]
fn rejects_group_selectors_and_nonchildren() {
    for pid in [0, u32::MAX] {
        assert_eq!(
            wait_owned_child(pid, WaitMode::Poll).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
    assert_eq!(
        wait_owned_child(std::process::id(), WaitMode::Poll)
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ECHILD)
    );
}

#[test]
fn terminal_status_preserves_exit_and_signal_without_reaping_siblings() {
    let mut sibling = Command::new("/bin/sh")
        .args(["-c", "exit 23"])
        .spawn()
        .unwrap();
    for script in ["exit 0", "exit 17", "kill -TERM $$"] {
        let mut child = Command::new("/bin/sh")
            .args(["-c", script])
            .spawn()
            .unwrap();
        let event = wait_owned_child(child.id(), WaitMode::Blocking)
            .unwrap()
            .unwrap();
        let usage = event.terminal_usage.unwrap().unwrap();
        assert!(usage.total_core_seconds >= 0.0);
        match script {
            "exit 0" => assert_eq!(event.status.code(), Some(0)),
            "exit 17" => assert_eq!(event.status.code(), Some(17)),
            _ => assert_eq!(event.status.signal(), Some(libc::SIGTERM)),
        }
        // The native waiter, rather than std::Child, was the sole reaper.
        assert_eq!(child.wait().unwrap_err().raw_os_error(), Some(libc::ECHILD));
    }
    assert_eq!(sibling.wait().unwrap().code(), Some(23));
}

#[test]
fn stop_and_continue_never_publish_terminal_usage() {
    let started = Instant::now();
    let mut child = Command::new("/bin/sh")
        .args(["-c", "kill -STOP $$; sleep 0.08; exit 17"])
        .spawn()
        .unwrap();
    let stopped = wait_owned_child(child.id(), WaitMode::Blocking)
        .unwrap()
        .unwrap();
    assert_eq!(stopped.status.stopped_signal(), Some(libc::SIGSTOP));
    assert!(stopped.terminal_usage.is_none());
    // SAFETY: this test owns the stopped child and resumes only its PID.
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGCONT) }, 0);
    loop {
        let event = wait_owned_child(child.id(), WaitMode::Blocking)
            .unwrap()
            .unwrap();
        if let Some(usage) = event.terminal_usage {
            assert!(usage.is_ok());
            assert_eq!(event.status.code(), Some(17));
            break;
        }
        assert!(event.status.continued());
    }
    assert!(started.elapsed().as_secs_f64() >= 0.08);
    assert_eq!(child.wait().unwrap_err().raw_os_error(), Some(libc::ECHILD));
}

#[test]
fn polling_is_nonblocking_and_does_not_consume_usage_early() {
    let mut child = Command::new("/bin/sleep").arg("0.2").spawn().unwrap();
    assert!(wait_owned_child(child.id(), WaitMode::Poll)
        .unwrap()
        .is_none());
    let terminal = wait_owned_child(child.id(), WaitMode::Blocking)
        .unwrap()
        .unwrap();
    assert!(terminal.status.success());
    assert!(terminal.terminal_usage.unwrap().is_ok());
    assert_eq!(child.wait().unwrap_err().raw_os_error(), Some(libc::ECHILD));
}
