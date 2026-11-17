use super::*;
use std::{io::Write, process::Stdio, sync::mpsc};

#[test]
fn expired_fixture_watchdog_terminates_and_reaps_its_child() {
    // Keep cat blocked on an open input peer; force expiry so the test checks
    // the watchdog path without waiting for the production 60-second guard.
    let (input, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
    let input: OwnedFd = input.into();
    let mut command = Command::new("/bin/cat");
    command
        .stdin(Stdio::from(input))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = GoChild::spawn(&mut command).unwrap();
    let pid = child.child.id();
    let error = child.wait_until(Instant::now()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(error.to_string().contains(&pid.to_string()), "{error}");
    assert!(child.reaped, "owned child was not reaped");
    assert!(!child.child.try_wait().unwrap().unwrap().success());
}

#[test]
fn cleanup_reaps_live_root_when_observation_or_tree_kill_fails() {
    for fail_observation in [true, false] {
        let mut command = Command::new("/bin/cat");
        command
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = GoChild::spawn(&mut command).unwrap();
        child
            .child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"ready\n")
            .unwrap();
        let mut stdout = child.child.stdout.take();
        nonblocking(stdout.as_ref()).unwrap();
        let events =
            OutputEvents::new(stdout.as_ref(), None::<&std::process::ChildStderr>).unwrap();
        let end = Instant::now() + Duration::from_secs(60);
        let mut received = Vec::new();
        loop {
            drain(&mut stdout, &mut received).unwrap();
            if received == b"ready\n" {
                break;
            }
            assert!(
                child.child.try_wait().unwrap().is_none(),
                "cat exited before readiness"
            );
            assert!(Instant::now() < end, "cat never echoed readiness");
            events.wait(end).unwrap();
        }
        let mut kill_called = false;
        let error = child
            .stop_with(
                |child| {
                    if fail_observation {
                        Err(io::Error::other("injected observation failure"))
                    } else {
                        child.observe()
                    }
                },
                |identity, depth| {
                    kill_called = true;
                    if fail_observation {
                        crate::cleanup::kill_tree(identity, depth)
                    } else {
                        vec!["injected tree termination failure".into()]
                    }
                },
            )
            .unwrap_err();
        assert!(kill_called, "the recorded root was not visited");
        let expected = if fail_observation {
            "injected observation failure"
        } else {
            "injected tree termination failure"
        };
        assert!(error.to_string().contains(expected), "{error}");
        assert!(child.reaped, "owned root was not reaped");
        assert!(!child.child.try_wait().unwrap().unwrap().success());
    }
}

#[test]
fn go_owned_child_counts_are_isolated_between_parallel_cases() {
    let (ready_tx, ready_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let mut releases = Vec::new();
    let mut workers = Vec::new();
    for _ in 0..2 {
        let (release, receive) = mpsc::channel();
        releases.push(release);
        let ready = ready_tx.clone();
        let done = done_tx.clone();
        workers.push(thread::spawn(move || {
            let before = counts();
            let mut command = Command::new("/bin/cat");
            command
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let mut child = GoChild::spawn(&mut command).unwrap();
            ready.send(()).unwrap();
            receive
                .recv_timeout(Duration::from_secs(60))
                .expect("case was not released");
            assert!(child.wait_with_output().unwrap().status.success());
            let after = counts();
            assert_eq!((after.0 - before.0, after.1 - before.1), (1, 1));
            done.send(()).unwrap();
        }));
    }
    for _ in 0..2 {
        ready_rx
            .recv_timeout(Duration::from_secs(60))
            .expect("case never started its owned child");
    }
    releases[0].send(()).unwrap();
    done_rx
        .recv_timeout(Duration::from_secs(60))
        .expect("first case never reaped its child");
    releases[1].send(()).unwrap();
    done_rx
        .recv_timeout(Duration::from_secs(60))
        .expect("second case never reaped its child");
    for worker in workers {
        worker.join().unwrap();
    }
}
