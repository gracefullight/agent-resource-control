use super::*;

fn accept_owned(
    listener: &std::os::unix::net::UnixListener,
    child: &mut crate::native_fixture::OwnedChild,
) -> Result<std::os::unix::net::UnixStream, String> {
    accept_owned_with_timeout(listener, child, Duration::from_secs(60))
}

fn accept_owned_with_timeout(
    listener: &std::os::unix::net::UnixListener,
    child: &mut crate::native_fixture::OwnedChild,
    timeout: Duration,
) -> Result<std::os::unix::net::UnixStream, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let end = Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).map_err(|e| e.to_string())?;
                return Ok(stream);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if let Some(status) = child.child.try_wait().map_err(|e| e.to_string())? {
                    return Err(format!("helper exited before registration: {status}"));
                }
                if Instant::now() >= end {
                    return Err("helper never connected for registration".into());
                }
                std::thread::yield_now();
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[test]
fn live_helper_without_registration_reports_the_watchdog() {
    use std::{
        fs,
        os::unix::net::UnixListener,
        process::{Command, Stdio},
    };
    let root = arc_core::runtime_root().unwrap().join(format!(
        "rs-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&root).unwrap();
    let listener = UnixListener::bind(root.join("ipc")).unwrap();
    let mut command = Command::new("/bin/cat");
    command
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = crate::native_fixture::OwnedChild::new(command.spawn().unwrap());
    let error = accept_owned_with_timeout(&listener, &mut child, Duration::ZERO).unwrap_err();
    assert!(error.contains("never connected for registration"));
    assert!(child.child.try_wait().unwrap().is_none());
    drop(child);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_helper_is_reported_without_waiting_for_a_connection() {
    use std::{fs, os::unix::net::UnixListener, process::Command};
    let root = arc_core::runtime_root().unwrap().join(format!(
        "re-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&root).unwrap();
    let listener = UnixListener::bind(root.join("ipc")).unwrap();
    let mut child =
        crate::native_fixture::OwnedChild::new(Command::new("/usr/bin/false").spawn().unwrap());
    assert!(accept_owned(&listener, &mut child)
        .unwrap_err()
        .contains("helper exited"));
    drop(child);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn delayed_bypass_and_stalled_registration_start_one_child_without_report() {
    use std::{
        cell::Cell,
        fs,
        io::{Read, Write},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{
                fs::{symlink, DirBuilderExt, PermissionsExt},
                net::{UnixListener, UnixStream},
                process::CommandExt,
            },
        },
        process::{Command, Stdio},
        rc::Rc,
    };
    const NAME: &str = "exec::transport::preparation_tests::stalled_registration::delayed_bypass_and_stalled_registration_start_one_child_without_report";
    const HELLO: &[u8] = b"arc-stalled-registration-test-v1";
    let mut peek = [0; HELLO.len()];
    let count = unsafe {
        libc::recv(
            9,
            peek.as_mut_ptr().cast(),
            peek.len(),
            libc::MSG_PEEK | libc::MSG_DONTWAIT,
        )
    };
    if count == HELLO.len() as isize && peek == HELLO {
        let mut peer = unsafe { UnixStream::from_raw_fd(9) };
        peer.read_exact(&mut peek).unwrap();
        let bun = std::env::var("ARC_STALLED_ROUTE").unwrap() == "bun";
        let clock = Rc::new(Cell::new(Instant::now()));
        let current = clock.clone();
        let deadline = Rc::new(Cell::new(None));
        let original = deadline.clone();
        let stage = Rc::new(Cell::new(0));
        let observed = stage.clone();
        let socket = PathBuf::from(std::env::var_os(arc_core::ARC_SOCKET_PATH).unwrap());
        let expired = PathBuf::from(std::env::var_os("ARC_STALL_CLOCK").unwrap());
        EXCHANGE.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move |operation, payload, end, cancelled| {
                let expected_probe = if bun { "traffic.bun.capabilities" } else { "traffic.capabilities" };
                let expected_job = if bun { "traffic.bun.job" } else { "traffic.job" };
                if operation == expected_probe {
                    original.set(Some(end));
                    Ok(serde_json::json!({"capability": if bun { "traffic.bun.v1" } else { "traffic.v1" }}))
                } else if operation == expected_job {
                    assert_eq!(Some(end), original.get());
                    peer.write_all(b"B").unwrap();
                    let mut release = [0];
                    peer.read_exact(&mut release).unwrap();
                    assert_eq!(release, *b"b");
                    current.set(end - Duration::from_millis(1));
                    observed.set(1);
                    if bun { Err(Failure::Unsupported) } else {
                        Ok(serde_json::json!(arc_ipc::traffic::JobResult::Bypassed { reason: arc_ipc::traffic::BypassReason::Unsupported }))
                    }
                } else if operation == "job.start" {
                    assert_eq!(payload["action"], "REGISTER");
                    assert_eq!(Some(end), original.get());
                    assert_eq!(observed.get(), 1);
                    assert!(current.get() < end);
                    let mut request = arc_ipc::Request::new("stalled-register", operation);
                    request.payload = payload.clone();
                    let clock = || if expired.exists() { end + Duration::from_millis(1) } else { current.get() };
                    let mut possibly_committed = false;
                    let result = arc_ipc::client::exchange_until_with_clock(
                        &socket, &request, end, cancelled, &mut possibly_committed,
                        arc_ipc::client::ExchangeOptions { continuity: None, retry_busy: true }, &clock,
                    );
                    assert_eq!(result, Err(Failure::Uncertain));
                    assert!(possibly_committed && expired.exists());
                    observed.set(2);
                    result
                } else {
                    panic!("unexpected operation after unconfirmed registration: {operation}");
                }
            }));
        });
        let _guard = ScriptedExchangeGuard;
        let argv = if bun {
            vec!["bun".into(), "test".into()]
        } else {
            vec!["cargo".into(), "build".into()]
        };
        assert_eq!(
            crate::exec::run_with_environment(argv, false, &|| clock.get()),
            std::process::ExitCode::from(17)
        );
        assert_eq!(stage.get(), 2);
        return;
    }
    for (route, script) in [("cargo", "build"), ("bun", "test")] {
        let root = arc_core::runtime_root().unwrap().join(format!(
            "sb-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        symlink("/bin/sh", root.join(route)).unwrap();
        fs::write(
            root.join(script),
            "printf 'start\\n' >> \"$ARC_STALL_MARKER\"\nexit 17\n",
        )
        .unwrap();
        let marker = root.join("starts");
        let clock = root.join("expired");
        let (mut controller, peer) = UnixStream::pair().unwrap();
        controller
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        controller.write_all(HELLO).unwrap();
        let fd = peer.as_raw_fd();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .env_clear()
            .args(["--exact", NAME, "--nocapture"])
            .env("ARC_STALLED_ROUTE", route)
            .env("ARC_STALL_MARKER", &marker)
            .env("ARC_STALL_CLOCK", &clock)
            .env(arc_core::ARC_SOCKET_PATH, root.join("ipc"))
            .env("PATH", &root)
            .env("HOME", &root)
            .env("TMPDIR", &root)
            .current_dir(&root)
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
        let mut helper = crate::native_fixture::OwnedChild::new(command.spawn().unwrap());
        drop(peer);
        let mut signal = [0];
        controller.read_exact(&mut signal).unwrap();
        assert_eq!(signal, *b"B");
        assert!(!marker.exists());
        // Governor has already fallen back; expose only the registration
        // socket while the Traffic bypass response is held.
        let listener = UnixListener::bind(root.join("ipc")).unwrap();
        fs::set_permissions(root.join("ipc"), fs::Permissions::from_mode(0o600)).unwrap();
        controller.write_all(b"b").unwrap();
        let mut registration = accept_owned(&listener, &mut helper).unwrap();
        registration
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        let request = arc_ipc::read_message::<_, arc_ipc::Request>(&mut registration).unwrap();
        assert_eq!(request.operation, "job.start");
        assert_eq!(request.payload["action"], "REGISTER");
        assert!(!marker.exists());
        let pending = root.join("expired.pending");
        fs::write(&pending, b"expired").unwrap();
        fs::rename(pending, &clock).unwrap();
        // Keep the registration socket open and send no reply. The client
        // observes the injected deadline on its next readiness check.
        assert!(
            helper.wait_with_output().unwrap().status.success(),
            "{route}"
        );
        assert_eq!(fs::read_to_string(&marker).unwrap(), "start\n");
        fs::remove_dir_all(root).unwrap();
    }
}
