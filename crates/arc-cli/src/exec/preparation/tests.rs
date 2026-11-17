use super::super::transport::Failure;
use super::admit;

const WRITER_HELLO: &[u8] = b"arc-notice-writer-test-v1";

#[test]
fn bounded_notice_waiter_uses_controlled_deadline_and_reaps_before_returning() {
    use std::{
        cell::Cell,
        time::{Duration, Instant},
    };
    let start = Instant::now();
    let current = Cell::new(start);
    let polls = Cell::new(0);
    let killed = Cell::new(false);
    let result = super::wait_notice_with(
        123,
        &|| current.get(),
        |pid, status, flags| {
            assert_eq!(pid, 123);
            if flags == libc::WNOHANG {
                polls.set(polls.get() + 1);
                Ok(0)
            } else {
                assert!(killed.get());
                *status = libc::SIGKILL;
                Ok(pid)
            }
        },
        |pid| {
            assert_eq!(pid, 123);
            killed.set(true);
        },
        || current.set(start + Duration::from_millis(50)),
    );
    assert_eq!(result, super::NoticeWrite::Uncertain);
    assert_eq!(polls.get(), 2);
    assert!(killed.get());

    for (status, expected) in [
        (0, super::NoticeWrite::Complete),
        (1 << 8, super::NoticeWrite::NotWritten),
        (2 << 8, super::NoticeWrite::Uncertain),
    ] {
        let result = super::wait_notice_with(
            123,
            &|| start,
            |pid, value, flags| {
                assert_eq!(flags, libc::WNOHANG);
                *value = status;
                Ok(pid)
            },
            |_| panic!("completed writer must not be killed"),
            || panic!("completed writer must not wait"),
        );
        assert_eq!(result, expected);
    }
}

// Only the test-owned descriptor and handshake can activate this helper.
#[test]
fn notice_writer_fixture() {
    use std::{
        io::{Read, Write},
        os::{fd::FromRawFd, unix::net::UnixStream},
    };
    let Some(mode) = std::env::var_os("ARC_NOTICE_WRITER_FIXTURE") else {
        return;
    };
    let mut peek = [0; WRITER_HELLO.len()];
    let received = unsafe {
        libc::recv(
            9,
            peek.as_mut_ptr().cast(),
            peek.len(),
            libc::MSG_PEEK | libc::MSG_DONTWAIT,
        )
    };
    if received != peek.len() as isize || peek != WRITER_HELLO {
        return;
    }
    let mut socket = unsafe { UnixStream::from_raw_fd(9) };
    socket.read_exact(&mut peek).unwrap();
    match mode.to_str().unwrap() {
        "closed" => {
            assert_eq!(unsafe { libc::close(libc::STDERR_FILENO) }, 0);
            assert_eq!(
                super::write_notice_for_fixture("queued"),
                super::NoticeWrite::NotWritten
            );
            socket.write_all(b"closed").unwrap();
            std::process::exit(0);
        }
        "none" => std::process::exit(1),
        "partial" => {
            socket.write_all(b"part").unwrap();
            std::process::exit(2);
        }
        "complete" => {
            socket.write_all(b"complete").unwrap();
            std::process::exit(0);
        }
        "killed" => {
            socket.write_all(b"complete").unwrap();
            let _ = socket.read_exact(&mut [0]);
            std::process::exit(3);
        }
        _ => std::process::exit(3),
    }
}

#[test]
fn closed_stderr_is_a_confirmed_non_emission() {
    use std::{
        io::{Read, Write},
        os::{
            fd::AsRawFd,
            unix::{net::UnixStream, process::CommandExt},
        },
        process::{Command, Stdio},
    };
    let (mut controller, peer) = UnixStream::pair().unwrap();
    controller
        .set_read_timeout(Some(std::time::Duration::from_secs(60)))
        .unwrap();
    controller.write_all(WRITER_HELLO).unwrap();
    let fd = peer.as_raw_fd();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .env("ARC_NOTICE_WRITER_FIXTURE", "closed")
        .args(["--exact", "exec::preparation::tests::notice_writer_fixture"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
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
    let mut child = crate::native_fixture::OwnedChild::new(command.spawn().unwrap());
    drop(peer);
    let mut ack = [0; 6];
    controller.read_exact(&mut ack).unwrap();
    assert_eq!(&ack, b"closed");
    assert!(child.wait_with_output().unwrap().status.success());
}

#[test]
fn notice_writer_classifies_synchronized_full_partial_and_killed_outputs() {
    use std::{
        io::{Read, Write},
        os::{
            fd::AsRawFd,
            unix::{
                net::UnixStream,
                process::{CommandExt, ExitStatusExt},
            },
        },
        process::{Command, Stdio},
        time::Duration,
    };
    for (mode, bytes, expected) in [
        ("none", &b""[..], super::NoticeWrite::NotWritten),
        ("partial", &b"part"[..], super::NoticeWrite::Uncertain),
        ("complete", &b"complete"[..], super::NoticeWrite::Complete),
        ("killed", &b"complete"[..], super::NoticeWrite::Uncertain),
    ] {
        let (mut controller, peer) = UnixStream::pair().unwrap();
        controller
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        controller.write_all(WRITER_HELLO).unwrap();
        let fd = peer.as_raw_fd();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .env_clear()
            .env("ARC_NOTICE_WRITER_FIXTURE", mode)
            .args(["--exact", "exec::preparation::tests::notice_writer_fixture"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        // SAFETY: only descriptor syscalls execute after fork.
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(fd, 9) < 0 || libc::fcntl(9, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = crate::native_fixture::OwnedChild::new(command.spawn().unwrap());
        drop(peer);
        let mut observed = vec![0; bytes.len()];
        controller.read_exact(&mut observed).unwrap();
        assert_eq!(observed, bytes);
        if mode == "killed" {
            child.child.kill().unwrap();
        }
        let status = child.wait_with_output().unwrap().status;
        assert_eq!(
            super::notice_writer_exit(status.into_raw()),
            expected,
            "{mode}"
        );
    }
}
#[test]
fn preparation_budget_is_exactly_1800_ms_from_injected_now() {
    let now = std::time::Instant::now();
    assert_eq!(
        super::deadline(now).duration_since(now),
        std::time::Duration::from_millis(1800)
    );
}
#[test]
fn cancellation_at_spawn_admission_cannot_invoke_spawn() {
    let mut invoked = false;
    let result = admit(
        || true,
        || {
            invoked = true;
        },
    );
    assert_eq!(result, Err(Failure::Cancelled));
    assert!(!invoked);
    assert_eq!(admit(|| false, || 17), Ok(17));
}
#[test]
fn nul_and_empty_native_arguments_are_invalid_before_metadata() {
    use std::os::unix::ffi::OsStringExt;
    assert_eq!(
        super::super::run(vec![std::ffi::OsString::from_vec(vec![b'x', 0])]),
        std::process::ExitCode::from(2)
    );
    assert_eq!(
        super::super::run(vec!["".into()]),
        std::process::ExitCode::from(2)
    );
}

#[test]
fn diagnostic_writes_are_best_effort_and_preserve_writable_output() {
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    super::write_diagnostic(
        &mut Broken,
        format_args!("TRACKING_UNAVAILABLE\nCommand started."),
    );
    let mut output = Vec::new();
    super::write_diagnostic(&mut output, format_args!("message {}", 17));
    assert_eq!(output, b"message 17\n");
}
