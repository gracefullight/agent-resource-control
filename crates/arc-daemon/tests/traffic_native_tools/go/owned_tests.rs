use super::*;
use std::{io::Write, process::Stdio, sync::mpsc};

#[test]
fn go_registers_detached_descendant_after_root_exits_before_observation() {
    use std::fs;
    let directory = std::env::temp_dir().join(format!(
        "arc-go-detached-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&directory).unwrap();
    let mut command = Command::new("/usr/bin/python3");
    command
        .args([
            "-c",
            r#"
import os, signal
read, write = os.pipe()
pid = os.fork()
if pid == 0:
    os.close(read)
    os.setsid()
    with open('child.pending', 'w') as f: f.write(str(os.getpid()))
    os.rename('child.pending', 'child')
    os.write(write, b'R')
    os.close(write)
    while True: signal.pause()
os.close(write)
assert os.read(read, 1) == b'R'
os._exit(0)
"#,
        ])
        .current_dir(&directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = GoChild::spawn(&mut command).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !directory.join("child").is_file() || !child.exited_without_reaping().unwrap() {
        assert!(
            Instant::now() < deadline,
            "detached Go descendant was not ready"
        );
        thread::yield_now();
    }
    let pid: u32 = fs::read_to_string(directory.join("child"))
        .unwrap()
        .parse()
        .unwrap();
    let identity = crate::cleanup::capture(pid).unwrap().unwrap();
    struct Guard(ProcessIdentity);
    impl Drop for Guard {
        fn drop(&mut self) {
            crate::cleanup::report_errors(crate::cleanup::kill_tree(&self.0, 0));
        }
    }
    let _guard = Guard(identity.clone());
    assert_ne!(
        unsafe { libc::getpgid(pid as i32) },
        child.child.id() as i32
    );
    let error = child.wait_until(Instant::now()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(
        child.identities().contains(&identity),
        "detached child was not registered"
    );
    assert_ne!(
        crate::cleanup::capture(pid).unwrap().as_ref(),
        Some(&identity)
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn go_preserves_detached_output_after_the_root_exits() {
    use std::{ffi::CString, fs, fs::OpenOptions, os::unix::fs::OpenOptionsExt};
    let directory = std::env::temp_dir().join(format!(
        "arc-go-detached-output-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&directory).unwrap();
    let fifo = CString::new(directory.join("gate").to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    let mut command = Command::new("/usr/bin/python3");
    command
        .args([
            "-c",
            r#"
import os
read, write = os.pipe(); pid = os.fork()
if pid == 0:
    os.close(read); os.setsid(); gate = os.open('gate', os.O_RDWR)
    with open('child.pending', 'w') as f: f.write(str(os.getpid()))
    os.rename('child.pending', 'child')
    os.write(write, b'R'); os.close(write)
    assert os.read(gate, 1) == b'R'
    os.write(1, b'detached Go output')
    os._exit(0)
os.close(write); assert os.read(read, 1) == b'R'; os._exit(0)
"#,
        ])
        .current_dir(&directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = GoChild::spawn(&mut command).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !directory.join("child").is_file() || !child.exited_without_reaping().unwrap() {
        assert!(
            Instant::now() < deadline,
            "detached Go output child was not ready"
        );
        thread::yield_now();
    }
    let pid: u32 = fs::read_to_string(directory.join("child"))
        .unwrap()
        .parse()
        .unwrap();
    let identity = crate::cleanup::capture(pid).unwrap().unwrap();
    struct Guard(ProcessIdentity);
    impl Drop for Guard {
        fn drop(&mut self) {
            crate::cleanup::report_errors(crate::cleanup::kill_tree(&self.0, 0));
        }
    }
    let _guard = Guard(identity.clone());
    child.observe().unwrap();
    assert!(child.identities().contains(&identity));
    let (started, ready) = mpsc::channel();
    let worker = thread::spawn(move || {
        started.send(()).unwrap();
        child.wait_with_output()
    });
    ready.recv_timeout(Duration::from_secs(60)).unwrap();
    let mut gate = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(directory.join("gate"))
        .unwrap();
    gate.write_all(b"R").unwrap();
    let output = worker.join().unwrap().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"detached Go output");
    assert!(output.stderr.is_empty());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn go_registers_a_pipe_holding_descendant_after_the_root_exits() {
    use std::fs;

    let directory = std::env::temp_dir().join(format!(
        "arc-go-owned-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&directory).unwrap();
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            r#"/usr/bin/mkfifo gate
exec 3<>gate
/bin/cat <&3 &
printf '%s' "$!" > child.pending
/bin/mv child.pending child
IFS= read -r release
exit 0"#,
        ])
        .current_dir(&directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = GoChild::spawn(&mut command).unwrap();
    assert_eq!(
        unsafe { libc::getpgid(child.child.id() as i32) },
        child.child.id() as i32
    );
    let deadline = Instant::now() + Duration::from_secs(60);
    while !directory.join("child").is_file() {
        assert!(
            Instant::now() < deadline,
            "Go descendant never published readiness"
        );
        thread::yield_now();
    }
    let pid: u32 = fs::read_to_string(directory.join("child"))
        .unwrap()
        .parse()
        .unwrap();
    let identity = crate::cleanup::child_identity(child.child.id(), pid)
        .unwrap()
        .unwrap();
    struct Guard(ProcessIdentity);
    impl Drop for Guard {
        fn drop(&mut self) {
            crate::cleanup::report_errors(crate::cleanup::kill_tree(&self.0, 0));
        }
    }
    let _guard = Guard(identity.clone());
    child
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"exit\n")
        .unwrap();
    loop {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    child.child.id(),
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            },
            0
        );
        if info.si_pid != 0 && info.si_code == libc::CLD_EXITED {
            break;
        }
        assert!(Instant::now() < deadline, "Go root never exited");
        thread::yield_now();
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    let observations = std::cell::Cell::new(0);
    let error = child
        .wait_with_clock(
            deadline,
            || {
                let current = observations.get();
                observations.set(current + 1);
                if current == 0 {
                    deadline - Duration::from_secs(1)
                } else {
                    deadline
                }
            },
            |_, _| Ok(()),
        )
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(
        child.identities().contains(&identity),
        "orphaned group member was not registered"
    );
    assert_ne!(
        crate::cleanup::capture(pid).unwrap().as_ref(),
        Some(&identity)
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn go_preserves_output_from_a_descendant_after_its_root_exits() {
    use std::{fs, fs::OpenOptions, os::unix::fs::OpenOptionsExt};

    let directory = std::env::temp_dir().join(format!(
        "arc-go-natural-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&directory).unwrap();
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            r#"/usr/bin/mkfifo gate
exec 3<>gate
/bin/sh -c 'IFS= read -r release <&3; printf "descendant output\n"' &
printf '%s' "$!" > child.pending
/bin/mv child.pending child
IFS= read -r release
exit 0"#,
        ])
        .current_dir(&directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = GoChild::spawn(&mut command).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !directory.join("child").is_file() {
        assert!(
            Instant::now() < deadline,
            "Go descendant never became ready"
        );
        thread::yield_now();
    }
    let mut release = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(directory.join("gate"))
        .unwrap();
    child
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"exit\n")
        .unwrap();
    while !child.exited_without_reaping().unwrap() {
        assert!(Instant::now() < deadline, "Go root never exited");
        thread::yield_now();
    }
    let (entered_tx, entered_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        entered_tx.send(()).unwrap();
        child.wait_with_output()
    });
    entered_rx.recv_timeout(Duration::from_secs(60)).unwrap();
    release.write_all(b"release\n").unwrap();
    drop(release);
    let output = worker.join().unwrap().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"descendant output\n");
    assert!(output.stderr.is_empty());
    fs::remove_dir_all(directory).unwrap();
}

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
        assert!(
            child.marker.is_retained(),
            "unverified ownership evidence was discarded"
        );
        assert!(child.marker.path().is_file());
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
