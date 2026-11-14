//! Actual daemon and CLI checks; native observations are separate from cap proofs.
#[path = "support/measurements.rs"]
mod measurements;
#[path = "support/native.rs"]
mod native;
#[path = "support/runtime.rs"]
mod runtime;
mod support;

use runtime::{assert_closed, receipt, send, Fixture};
use std::{
    io::Write,
    time::{Duration, Instant},
};

#[test]
fn malformed_and_stalled_frames_recover_actual_cli() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn("test");
    let mut idle = fixture.connect();
    idle.write_all(&[0]).unwrap();
    // Closure is observed, never inferred from a sleep. Paused-time units prove
    // the exact deadline; the subprocess only has a coverage-safe fixture watchdog.
    assert_closed(idle);
    let mut body = fixture.connect();
    body.write_all(&1024u32.to_be_bytes()).unwrap();
    body.write_all(b"{").unwrap();
    assert_closed(body);

    let mut stream = fixture.connect();
    stream.write_all(&1u32.to_be_bytes()).unwrap();
    stream.write_all(b"{").unwrap();
    let invalid: arc_ipc::Response = arc_ipc::read_message(&mut stream).unwrap();
    assert_eq!(invalid.error.unwrap().code, arc_ipc::ERROR_INVALID_REQUEST);
    let mut version = arc_ipc::Request::new("wrong", arc_ipc::OPERATION_DOCTOR);
    version.protocol_version = 99;
    let response = arc_ipc::exchange(&mut stream, &version).unwrap();
    assert_eq!(
        response.error.unwrap().code,
        arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE
    );
    let response = arc_ipc::exchange(
        &mut stream,
        &arc_ipc::Request::new("ok", arc_ipc::OPERATION_HELLO),
    )
    .unwrap();
    assert_eq!(response.status, arc_ipc::STATUS_OK);
    drop(stream);
    for (length, code) in [
        (0, arc_ipc::ERROR_INVALID_REQUEST),
        (arc_ipc::MAX_FRAME_BYTES + 1, arc_ipc::ERROR_FRAME_TOO_LARGE),
    ] {
        let mut stream = fixture.connect();
        stream.write_all(&length.to_be_bytes()).unwrap();
        let response: arc_ipc::Response = arc_ipc::read_message(&mut stream).unwrap();
        assert_eq!(response.error.unwrap().code, code);
        assert_closed(stream);
    }
    fixture.assert_cli();
    let (exit_us, peak) = child.stop(libc::SIGTERM);
    let report = fixture.report();
    assert_eq!(report["metrics"]["header_timeouts"], 1);
    assert_eq!(report["metrics"]["frame_timeouts"], 1);
    receipt(
        "frames",
        &serde_json::json!({"report":report,"signal_to_exit_us":exit_us,"native_peak_rss_bytes":peak}),
    );
}

#[test]
fn nonreading_peer_releases_real_daemon_capacity() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn("test");
    let mut peer = fixture.connect();
    let size: libc::c_int = 1024;
    use std::os::fd::AsRawFd;
    assert_eq!(
        unsafe {
            libc::setsockopt(
                peer.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_RCVBUF,
                &size as *const _ as *const libc::c_void,
                std::mem::size_of_val(&size) as libc::socklen_t,
            )
        },
        0
    );
    send(
        &mut peer,
        &arc_ipc::Request::new("x".repeat(512 * 1024), arc_ipc::OPERATION_HELLO),
    );
    // Leave the peer unread until its kernel reports EOF/HUP via poll. No bytes
    // are drained to create writable space while the server deadline expires.
    let started = Instant::now();
    loop {
        let mut poll = libc::pollfd {
            fd: peer.as_raw_fd(),
            events: libc::POLLHUP,
            revents: 0,
        };
        assert!(unsafe { libc::poll(&mut poll, 1, 10) } >= 0);
        if poll.revents & libc::POLLHUP != 0 {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "nonreading socket not closed"
        );
    }
    let closure_us = started.elapsed().as_micros();
    drop(peer);
    fixture.assert_cli();
    let (exit_us, _) = child.stop(libc::SIGTERM);
    let report = fixture.report();
    assert_eq!(
        report["metrics"]["write_timeouts"], 1,
        "actual backpressure required: {report}"
    );
    receipt(
        "nonreader",
        &serde_json::json!({"report":report,"closure_us":closure_us,"signal_to_exit_us":exit_us}),
    );
}

#[test]
fn native_probe_then_full_100000_attempt_workload() {
    measurements::run();
}

#[test]
fn owned_child_drop_kills_and_reaps_before_fixture_directory_removal_on_unwind() {
    let fixture = Fixture::new();
    let root = fixture.root.clone();
    let pid = std::cell::Cell::new(0);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut command = std::process::Command::new("/bin/sh");
        command
            .args(["-c", "read value"])
            .stdin(std::process::Stdio::piped());
        let child = runtime::OwnedChild::new(command.spawn().unwrap());
        pid.set(child.child.id());
        panic!("exercise owned fixture unwind");
    }));
    assert!(unwind.is_err());
    assert_eq!(
        unsafe { libc::waitpid(pid.get() as i32, std::ptr::null_mut(), libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
    assert!(root.exists());
    drop(fixture);
    assert!(!root.exists());
}

#[tokio::test]
async fn cancelled_fixture_future_reaps_its_owned_child() {
    let fixture = Fixture::new();
    let (entered, ready) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let child = runtime::OwnedChild::new(
            std::process::Command::new("/bin/sh")
                .args(["-c", "read value"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
        entered.send(child.child.id()).unwrap();
        std::future::pending::<()>().await;
        drop(child);
    });
    let pid = ready.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(
        unsafe { libc::waitpid(pid as i32, std::ptr::null_mut(), libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
    assert!(fixture.root.exists());
}

#[tokio::test]
async fn cancelled_fixture_before_identity_publication_reaps_wrapper_and_target() {
    use arc_core::process_identity::ProcessIdentity;
    let fixture = Fixture::new();
    let ready = fixture.root.join("before-identity");
    let owners = fixture.root.join("cancel-owners");
    let mut command = std::process::Command::new(fixture.binary("arcctl"));
    command
        .args(["exec", "--"])
        .arg(fixture.binary("arc-exec-fixture"))
        .env(arc_core::ARC_SOCKET_PATH, fixture.root.join("missing.sock"))
        .env("ARC_FIXTURE_ACCOUNTING", "stop")
        .env("ARC_FIXTURE_BEFORE_IDENTITY", &ready)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let (entered, received) = tokio::sync::oneshot::channel();
    let owned = owners.clone();
    let task = tokio::spawn(async move {
        let child = runtime::OwnedChild::spawn_with_descendants(command, owned).unwrap();
        entered.send(child.id()).unwrap();
        std::future::pending::<()>().await;
        drop(child);
    });
    let wrapper = received.await.unwrap();
    let end = Instant::now() + Duration::from_secs(60);
    let target = loop {
        if let Ok(pid) = std::fs::read_to_string(&ready)
            .unwrap_or_default()
            .parse::<u32>()
        {
            break ProcessIdentity::capture(pid).unwrap();
        }
        assert!(Instant::now() < end, "cancellation checkpoint");
        tokio::time::sleep(Duration::from_millis(1)).await;
    };
    assert_eq!(std::fs::read_dir(owners).unwrap().count(), 0);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(ProcessIdentity::capture(target.pid).is_err());
    assert_eq!(
        unsafe { libc::waitpid(wrapper as i32, std::ptr::null_mut(), libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
    assert!(fixture.root.exists());
}

#[test]
fn owned_child_collects_both_full_pipes_and_reaps_native_exit() {
    let mut command = std::process::Command::new("python3");
    command
        .args([
            "-c",
            "import os,sys; os.write(1,b'o'*262144); os.write(2,b'e'*262144); sys.exit(17)",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = runtime::OwnedChild::new(command.spawn().unwrap());
    let pid = child.child.id();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(17));
    assert_eq!(output.stdout, vec![b'o'; 262144]);
    assert_eq!(output.stderr, vec![b'e'; 262144]);
    drop(child);
    assert_eq!(
        unsafe { libc::waitpid(pid as i32, std::ptr::null_mut(), libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
}

#[test]
fn exited_parent_with_owned_pipe_holder_drains_bytes_and_cleans_up() {
    use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
    let fixture = Fixture::new();
    let mut child = runtime::OwnedChild::spawn_with_descendants(
        pipe_holder_command(&fixture),
        fixture.root.join("holder-owners"),
    )
    .unwrap();
    let end = Instant::now() + Duration::from_secs(60);
    pipe_holder_checkpoint(&fixture.root);
    child.stdin.as_mut().unwrap().write_all(b"a").unwrap();
    let holder = loop {
        if let Ok(bytes) = std::fs::read(fixture.root.join("holder-identity")) {
            if let Ok(identity) = serde_json::from_slice::<ProcessIdentity>(&bytes) {
                break identity;
            }
        }
        assert!(Instant::now() < end, "pipe holder readiness");
        std::thread::sleep(Duration::from_millis(1));
    };
    child.own_process(holder.clone());
    let output = child.wait_with_output().unwrap();
    assert!(
        Instant::now() < end,
        "complete exit/drain lifecycle exceeded fixture watchdog"
    );
    assert_eq!(output.status.code(), Some(17));
    assert_eq!(output.stdout, vec![b'o'; 262144]);
    assert_eq!(output.stderr, vec![b'e'; 262144]);
    while probe_live(&holder) == IdentityStatus::VerifiedMatch {
        assert!(Instant::now() < end, "owned pipe holder survived cleanup");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(probe_live(&holder), IdentityStatus::Missing);
}

fn pipe_holder_command(fixture: &Fixture) -> std::process::Command {
    let mut command = std::process::Command::new(fixture.binary("arc-exec-fixture"));
    command
        .env("ARC_FIXTURE_ACCOUNTING", "pipe-parent")
        .env(
            "ARC_FIXTURE_PIPE_CHECKPOINT",
            fixture.root.join("holder-checkpoint"),
        )
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    command
}

fn pipe_holder_checkpoint(root: &std::path::Path) {
    let end = Instant::now() + Duration::from_secs(60);
    while !root.join("holder-checkpoint").exists() {
        assert!(Instant::now() < end, "pre-adoption pipe holder checkpoint");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        std::fs::read_dir(root.join("holder-owners"))
            .unwrap()
            .count(),
        0
    );
    assert!(!root.join("holder-identity").exists());
}

fn assert_pipe_holder_cleanup(root: &std::path::Path, parent: u32) {
    use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("holder-cleanup")).unwrap()).unwrap();
    assert_eq!(receipt["reaped"], true);
    let holder: ProcessIdentity = serde_json::from_value(receipt["identity"].clone()).unwrap();
    assert_eq!(probe_live(&holder), IdentityStatus::Missing);
    assert!(
        ProcessIdentity::capture(holder.pid).is_err(),
        "holder still exists, possibly zombie"
    );
    assert_eq!(
        unsafe { libc::waitpid(parent as i32, std::ptr::null_mut(), libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
    assert!(!root.join("owned-cleanup-incomplete").exists());
    assert!(root.exists(), "reaping must precede fixture removal");
}

#[test]
fn pre_adoption_pipe_holder_readiness_failure_and_unwind_reap_both_children() {
    for unwind in [false, true] {
        let fixture = Fixture::new();
        let root = fixture.root.clone();
        let parent = std::cell::Cell::new(0);
        let failure =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> std::io::Result<()> {
                let child = runtime::OwnedChild::spawn_with_descendants(
                    pipe_holder_command(&fixture),
                    root.join("holder-owners"),
                )?;
                parent.set(child.id());
                pipe_holder_checkpoint(&root);
                if unwind {
                    panic!("readiness rejected before pipe-holder adoption");
                }
                Err(std::io::Error::other(
                    "injected readiness failure before adoption",
                ))
            }));
        if unwind {
            assert!(failure.is_err());
        } else {
            assert!(failure.unwrap().is_err());
        }
        assert_pipe_holder_cleanup(&root, parent.get());
        drop(fixture);
        assert!(!root.exists());
    }
}

#[tokio::test]
async fn pre_adoption_pipe_holder_cancellation_reaps_both_children() {
    let fixture = Fixture::new();
    let root = fixture.root.clone();
    let command = pipe_holder_command(&fixture);
    let owned_root = root.clone();
    let (entered, ready) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let child =
            runtime::OwnedChild::spawn_with_descendants(command, owned_root.join("holder-owners"))
                .unwrap();
        pipe_holder_checkpoint(&owned_root);
        entered.send(child.id()).unwrap();
        std::future::pending::<()>().await;
        drop(child);
    });
    let parent = ready.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_pipe_holder_cleanup(&root, parent);
    drop(fixture);
    assert!(!root.exists());
}

#[test]
fn stale_descendant_identity_cannot_reap_an_unrelated_child() {
    use arc_core::process_identity::ProcessIdentity;
    let mut peer = runtime::OwnedChild::new(
        std::process::Command::new("/bin/sh")
            .args(["-c", "read value; exit 0"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut stale = ProcessIdentity::capture(peer.id()).unwrap();
    stale.start_token = (stale.start_token.parse::<u64>().unwrap() + 1).to_string();
    drop(peer.stdin.take());
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        let mut event = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
        assert_eq!(
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    peer.id(),
                    &mut event,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            },
            0
        );
        if event.si_code == libc::CLD_EXITED {
            break;
        }
        assert!(Instant::now() < end, "peer exit acknowledgement");
        std::thread::yield_now();
    }
    let mut owner =
        runtime::OwnedChild::new(std::process::Command::new("/usr/bin/true").spawn().unwrap());
    assert!(owner.wait().unwrap().success());
    owner.own_process(stale);
    drop(owner);
    assert!(
        peer.wait().unwrap().success(),
        "a mismatched identity is not cleanup ownership"
    );
}
