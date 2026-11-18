use super::*;

#[test]
fn fixture_unwind_cleans_detached_child_after_root_exits_before_observation() {
    use std::time::Instant;
    let fixture = Fixture::new();
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
    with open('detached.pending', 'w') as f: f.write(str(os.getpid()))
    os.rename('detached.pending', 'detached-ready')
    os.write(write, b'R')
    os.close(write)
    while True: signal.pause()
os.close(write)
assert os.read(read, 1) == b'R'
os._exit(0)
"#,
        ])
        .current_dir(&fixture.root)
        .stdin(Stdio::null());
    let mut child = FixtureChild::spawn(command, &fixture.root);
    let root = child.child.child_mut().id();
    let deadline = Instant::now() + Duration::from_secs(60);
    let ready = fixture.root.join("detached-ready");
    loop {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    root,
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            },
            0
        );
        #[cfg(target_os = "macos")]
        let exited = info.si_pid != 0;
        #[cfg(target_os = "linux")]
        let exited = unsafe { info.si_pid() != 0 };
        if ready.is_file() && exited {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "detached hook child was not ready"
        );
        thread::yield_now();
    }
    let pid: u32 = fs::read_to_string(ready).unwrap().parse().unwrap();
    let identity = arc_core::process_identity::ProcessIdentity::capture(pid).unwrap();
    struct Guard(arc_core::process_identity::ProcessIdentity);
    impl Drop for Guard {
        fn drop(&mut self) {
            if arc_core::process_identity::ProcessIdentity::capture(self.0.pid)
                .ok()
                .as_ref()
                == Some(&self.0)
            {
                unsafe {
                    libc::kill(self.0.pid as i32, libc::SIGKILL);
                }
            }
        }
    }
    let _guard = Guard(identity.clone());
    assert_ne!(unsafe { libc::getpgid(pid as i32) }, root as i32);
    drop(child);
    assert_eq!(
        arc_core::process_identity::probe(&identity),
        arc_core::process_identity::IdentityStatus::Missing
    );
}

#[test]
fn fixture_preserves_detached_output_after_the_root_exits() {
    use std::{ffi::CString, fs::OpenOptions, os::unix::fs::OpenOptionsExt, time::Instant};
    let fixture = Fixture::new();
    let fifo = CString::new(fixture.root.join("gate").to_str().unwrap()).unwrap();
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
    with open('detached.pending', 'w') as f: f.write(str(os.getpid()))
    os.rename('detached.pending', 'detached-ready')
    os.write(write, b'R'); os.close(write)
    assert os.read(gate, 1) == b'R'
    os.write(1, b'detached hook output')
    os._exit(0)
os.close(write); assert os.read(read, 1) == b'R'; os._exit(0)
"#,
        ])
        .current_dir(&fixture.root)
        .stdin(Stdio::null());
    let mut child = FixtureChild::spawn(command, &fixture.root);
    let root = child.child.child_mut().id();
    let deadline = Instant::now() + Duration::from_secs(60);
    let ready = fixture.root.join("detached-ready");
    loop {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    root,
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            },
            0
        );
        #[cfg(target_os = "macos")]
        let exited = info.si_pid != 0;
        #[cfg(target_os = "linux")]
        let exited = unsafe { info.si_pid() != 0 };
        if ready.is_file() && exited {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "detached hook output child was not ready"
        );
        thread::yield_now();
    }
    let pid: u32 = fs::read_to_string(ready).unwrap().parse().unwrap();
    let identity = arc_core::process_identity::ProcessIdentity::capture(pid).unwrap();
    struct Guard(arc_core::process_identity::ProcessIdentity);
    impl Drop for Guard {
        fn drop(&mut self) {
            if arc_core::process_identity::ProcessIdentity::capture(self.0.pid)
                .ok()
                .as_ref()
                == Some(&self.0)
            {
                unsafe {
                    libc::kill(self.0.pid as i32, libc::SIGKILL);
                }
            }
        }
    }
    let _guard = Guard(identity);
    assert!(
        child.child.observe().unwrap(),
        "detached child was not registered"
    );
    let (started, received) = mpsc::channel();
    let worker = thread::spawn(move || {
        started.send(()).unwrap();
        child.output()
    });
    received.recv_timeout(Duration::from_secs(60)).unwrap();
    let mut gate = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(fixture.root.join("gate"))
        .unwrap();
    gate.write_all(b"R").unwrap();
    let output = worker.join().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"detached hook output");
    assert!(output.stderr.is_empty());
}

fn exited_shell_with_nested_child(
    fixture: &Fixture,
    complete: bool,
) -> (FixtureChild, u32, fs::File) {
    use std::{fs::OpenOptions, os::unix::fs::OpenOptionsExt, time::Instant};
    let nested = if complete {
        "/bin/sh -c 'IFS= read -r release <&3; printf nested-output'"
    } else {
        "/bin/cat <&3"
    };
    let script = format!(
        r#"/usr/bin/mkfifo nested-gate
exec 3<>nested-gate
{nested} &
printf '%s' "$!" > nested-pending
/bin/mv nested-pending nested-ready
IFS= read -r release
exit 0"#
    );
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", &script])
        .current_dir(&fixture.root)
        .stdin(Stdio::piped());
    let mut child = FixtureChild::spawn(command, &fixture.root);
    let deadline = Instant::now() + Duration::from_secs(60);
    let ready = fixture.root.join("nested-ready");
    while !ready.is_file() {
        assert!(
            Instant::now() < deadline,
            "nested hook child never became ready"
        );
        thread::yield_now();
    }
    let pid = fs::read_to_string(ready).unwrap().parse().unwrap();
    let gate = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(fixture.root.join("nested-gate"))
        .unwrap();
    let root = child.child.child_mut().id();
    child.write_input(b"exit\n");
    loop {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    root,
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            },
            0
        );
        #[cfg(target_os = "macos")]
        let observed = info.si_pid;
        #[cfg(target_os = "linux")]
        let observed = unsafe { info.si_pid() };
        if observed != 0 {
            break;
        }
        assert!(Instant::now() < deadline, "hook shell never exited");
        thread::yield_now();
    }
    (child, pid, gate)
}

#[test]
fn fixture_waits_for_nested_output_after_the_shell_exits() {
    let fixture = Fixture::new();
    let (mut child, pid, mut gate) = exited_shell_with_nested_child(&fixture, true);
    let (started, ready) = mpsc::channel();
    let worker = thread::spawn(move || {
        started.send(()).unwrap();
        child.output()
    });
    ready.recv_timeout(Duration::from_secs(60)).unwrap();
    gate.write_all(b"complete\n").unwrap();
    drop(gate);
    let output = worker.join().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"nested-output");
    assert!(output.stderr.is_empty());
    assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}

#[test]
fn fixture_unwind_cleans_nested_child_after_the_shell_exits() {
    let fixture = Fixture::new();
    let (child, pid, _gate) = exited_shell_with_nested_child(&fixture, false);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _child = child;
        panic!("exercise nested fixture cleanup after root exit");
    }));
    assert!(result.is_err());
    assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}
