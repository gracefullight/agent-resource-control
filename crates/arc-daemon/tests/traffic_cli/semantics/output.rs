use super::*;
use std::{
    io::{self, Read},
    os::fd::AsRawFd,
    process::Output,
};

pub(super) fn managed_output(f: &Fixture, command: &mut Command, description: &str) -> Output {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = BuildChild::new(OwnedChild::new(command.spawn().unwrap()), f, true);
    let deadline = Instant::now() + Duration::from_secs(60);
    let result = wait_output(&mut child, || Instant::now() >= deadline);
    // BuildChild owns the native tree while the wrapper is still unreaped.
    drop(child);
    result.unwrap_or_else(|error| panic!("{description}: {error}"))
}

fn nonblocking(pipe: Option<&impl AsRawFd>) -> io::Result<()> {
    if let Some(pipe) = pipe {
        let fd = pipe.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

fn drain(pipe: &mut Option<impl Read>, bytes: &mut Vec<u8>) -> io::Result<()> {
    let mut buffer = [0; 16384];
    for _ in 0..16 {
        let Some(reader) = pipe.as_mut() else {
            break;
        };
        match reader.read(&mut buffer) {
            Ok(0) => {
                *pipe = None;
                break;
            }
            Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(super) fn wait_output(
    wrapper: &mut BuildChild,
    mut expired: impl FnMut() -> bool,
) -> io::Result<Output> {
    let mut out = wrapper.child.stdout.take();
    let mut err = wrapper.child.stderr.take();
    nonblocking(out.as_ref())?;
    nonblocking(err.as_ref())?;
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    loop {
        drain(&mut out, &mut stdout)?;
        drain(&mut err, &mut stderr)?;
        // Do not reap the wrapper while a native child may still hold a pipe.
        // On error, BuildChild's owner can then terminate the live tree first.
        if out.is_none() && err.is_none() {
            if let Some(status) = wrapper.child.try_wait()? {
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
        }
        if expired() {
            let tail = &stderr[stderr.len().saturating_sub(4096)..];
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "managed child exit/pipe watchdog; stderr tail: {}",
                    String::from_utf8_lossy(tail)
                ),
            ));
        }
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn stalled_fail_open_child_is_terminated_before_wrapper_reap() {
    use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};

    let _serial = support::serial();
    let f = Fixture::new();
    let peer_listener = UnixListener::bind(&f.socket).unwrap();
    peer_listener.set_nonblocking(true).unwrap();
    let peer = thread::spawn(move || {
        for _ in 0..3 {
            let (_, request) = policy_compat::next_legacy(&peer_listener);
            assert_eq!(
                request.operation,
                arc_ipc::traffic::OPERATION_BUN_CAPABILITIES
            );
        }
    });
    let control_path = f.root.join("child-control");
    let control = UnixListener::bind(&control_path).unwrap();
    control.set_nonblocking(true).unwrap();
    symlink(binary("arc-exec-fixture"), f.root.join("bun")).unwrap();
    let marker = f.root.join("child-marker");
    let mut command = Command::new(binary("arcctl"));
    command
        .args(["exec", "--", "bun", "test"])
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env("PATH", &f.root)
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", &marker)
        .env("ARC_FIXTURE_CONTROL", &control_path)
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut wrapper = BuildChild::new(OwnedChild::new(command.spawn().unwrap()), &f, true);
    let deadline = Instant::now() + Duration::from_secs(60);
    let _connection = loop {
        match control.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "fail-open child did not connect");
                thread::yield_now();
            }
            Err(error) => panic!("control accept: {error}"),
        }
    };
    let pid: u32 = fs::read_to_string(&marker).unwrap().trim().parse().unwrap();
    let identity = ProcessIdentity::capture(pid).unwrap();
    assert_eq!(probe_live(&identity), IdentityStatus::VerifiedMatch);
    // Trigger the deadline after observing the real native child. The normal
    // caller supplies a 60-second monotonic watchdog to the same waiter.
    let result = wait_output(&mut wrapper, || true);
    drop(wrapper);
    let peer_result = peer.join();
    let deadline = Instant::now() + Duration::from_secs(60);
    let remaining = loop {
        let status = probe_live(&identity);
        if matches!(status, IdentityStatus::Missing | IdentityStatus::Mismatch) {
            break None;
        }
        if Instant::now() >= deadline {
            if status == IdentityStatus::VerifiedMatch {
                // A failing regression must not leave its own orphan running.
                unsafe { libc::kill(pid as i32, libc::SIGKILL) };
            }
            break Some(status);
        }
        thread::yield_now();
    };
    peer_result.unwrap();
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
    assert!(
        remaining.is_none(),
        "native child cleanup unconfirmed: {remaining:?}"
    );
}
