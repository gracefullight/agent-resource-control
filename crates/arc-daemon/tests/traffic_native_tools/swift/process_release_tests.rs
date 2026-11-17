//! Validate the sibling helper's release protocol and cleanup on failure.
use super::*;

#[test]
fn swift_stopped_leader_is_not_treated_as_exited() {
    let mut tree = HeldTree::start();
    tree.runner.signal_group(libc::SIGSTOP).unwrap();
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    tree.runner.root.pid,
                    &mut info,
                    libc::WSTOPPED | libc::WNOHANG | libc::WNOWAIT,
                )
            },
            0,
            "waitid stopped leader: {}",
            io::Error::last_os_error()
        );
        if info.si_pid != 0 && info.si_code == libc::CLD_STOPPED {
            break;
        }
        assert!(
            Instant::now() < end,
            "leader never reported its stopped state"
        );
        std::thread::yield_now();
    }
    assert!(!tree.runner.has_exited().unwrap());
    tree.runner.cleanup().unwrap();
    assert_gone(&tree.identities);
    drop(tree.runner);
    fs::remove_dir_all(tree.directory).unwrap();
}

#[test]
fn swift_group_termination_keeps_the_leader_owned_and_reports_denial() {
    for deny_group in [false, true] {
        let mut tree = HeldTree::start();
        let mut signalled = false;
        let result = tree.runner.cleanup_with_termination(
            |child| child.signal_group(libc::SIGSTOP),
            &mut |_| Ok(()),
            &mut crate::cleanup::capture,
            |child| {
                signalled = true;
                assert!(
                    !child.exited_without_reaping().unwrap(),
                    "the group leader must remain alive until its group is terminated"
                );
                let mut state: libc::siginfo_t = unsafe { std::mem::zeroed() };
                assert_eq!(
                    unsafe {
                        libc::waitid(
                            libc::P_PID,
                            child.root.pid,
                            &mut state,
                            libc::WSTOPPED | libc::WNOHANG | libc::WNOWAIT,
                        )
                    },
                    0
                );
                assert_eq!(state.si_pid, child.root.pid as i32);
                assert_eq!(state.si_code, libc::CLD_STOPPED);
                if deny_group {
                    Err(io::Error::from_raw_os_error(libc::EPERM))
                } else {
                    child.signal_group(libc::SIGKILL)
                }
            },
        );
        assert!(signalled);
        if deny_group {
            let permission_denied = io::Error::from_raw_os_error(libc::EPERM).to_string();
            assert!(
                result.unwrap_err().to_string().contains(&permission_denied),
                "group permission errors must remain visible"
            );
        } else {
            result.unwrap();
        }
        assert!(tree.runner.cleaned);
        assert_gone(&tree.identities);
        drop(tree.runner);
        fs::remove_dir_all(tree.directory).unwrap();
    }
}

#[test]
fn swift_helper_rejects_closed_release_channel_and_reaps_siblings() {
    let (mut runner, channel, identities) = separate_group_siblings();
    drop(channel);
    let output = runner.wait_with_output_until_exit().unwrap();
    assert_gone(&identities);
    assert!(
        !output.status.success(),
        "sibling helper succeeded after the release channel closed: {output:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("sibling helper release failed"),
        "missing release failure diagnostic: {stderr}"
    );
}

#[test]
fn swift_helper_rejects_unexpected_release_byte_and_reaps_siblings() {
    let (mut runner, mut channel, identities) = separate_group_siblings();
    channel.write_all(b"?").unwrap();
    let output = runner.wait_with_output_until_exit().unwrap();
    assert_gone(&identities);
    assert!(
        !output.status.success(),
        "sibling helper accepted an unexpected release byte: {output:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unexpected sibling release byte"),
        "missing invalid release diagnostic: {stderr}"
    );
}

#[test]
fn swift_helper_release_read_errors_report_failure() {
    struct FailedRead(io::ErrorKind);
    impl Read for FailedRead {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(self.0, "injected release read failure"))
        }
    }

    // Platforms can expose a socket watchdog as either TimedOut or WouldBlock.
    for kind in [io::ErrorKind::TimedOut, io::ErrorKind::WouldBlock] {
        let error = receive_sibling_release(&mut FailedRead(kind)).unwrap_err();
        assert_eq!(error.kind(), kind);
        assert!(error
            .to_string()
            .contains("failed to read sibling release byte"));
        assert!(error.to_string().contains("injected release read failure"));
    }
}
