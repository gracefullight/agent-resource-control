use super::*;
use std::{fs, process::Stdio};

#[path = "../../../arc-daemon/tests/support/instrumentation.rs"]
mod instrumentation;

#[test]
fn transient_identity_unavailability_waits_without_treating_it_as_absence() {
    let directory = std::env::temp_dir().join(format!(
        "arc-hook-transient-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&directory).unwrap();
    let mut command = Command::new("/usr/bin/true");
    command
        .env_clear()
        .env("HOME", &directory)
        .current_dir(&directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    instrumentation::forward(&mut command);
    let mut child = OwnedChild::spawn(&mut command, "transient identity observation");
    let mut calls = 0;
    let status = child.wait_with_observer(Instant::now, |child| {
        calls += 1;
        if calls == 1 {
            Err(io::Error::other("process identity unavailable"))
        } else {
            child.observe()
        }
    });
    assert!(status.success());
    assert!(calls >= 2, "unknown identity must not complete the wait");
    drop(child);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn failed_marker_observation_cleans_group_and_retains_evidence() {
    let directory = std::env::temp_dir().join(format!(
        "arc-hook-observation-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&directory).unwrap();
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            "/bin/cat <&0 >/dev/null 2>&1 & printf '%s' \"$!\" > child.pending; /bin/mv child.pending child; wait",
        ])
        .current_dir(&directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = OwnedChild::spawn(&mut command, "injected observation failure");
    let root = child.child_mut().id();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !directory.join("child").is_file() {
        assert!(!child.root_exited().unwrap());
        assert!(Instant::now() < deadline, "owned group child was not ready");
        thread::yield_now();
    }
    let pid: u32 = fs::read_to_string(directory.join("child"))
        .unwrap()
        .parse()
        .unwrap();
    let identity = ProcessIdentity::capture(pid).unwrap();
    struct Guard(ProcessIdentity);
    impl Drop for Guard {
        fn drop(&mut self) {
            if ProcessIdentity::capture(self.0.pid).ok().as_ref() == Some(&self.0) {
                unsafe { libc::kill(self.0.pid as i32, libc::SIGKILL) };
            }
        }
    }
    let _guard = Guard(identity.clone());
    assert_eq!(unsafe { libc::getpgid(pid as i32) }, root as i32);
    let marker = child.marker.as_ref().unwrap().path().to_owned();
    let error = child
        .cleanup_with(|_| Err(io::Error::other("injected marker observation failure")))
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("injected marker observation failure"));
    assert!(child.child.is_none(), "owned root was not reaped");
    assert_ne!(ProcessIdentity::capture(pid).ok().as_ref(), Some(&identity));
    assert!(child.marker.as_ref().unwrap().is_retained());
    drop(child);
    assert!(
        marker.is_file(),
        "recovery marker was removed after failed observation"
    );
    fs::remove_file(marker).unwrap();
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn hook_watchdog_reaps_a_detached_child_after_inherited_descriptors_close() {
    let directory = std::env::temp_dir().join(format!(
        "arc-hook-closed-fds-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&directory).unwrap();
    let mut command = Command::new("/usr/bin/python3");
    command
        .args([
            "-c",
            r#"
import os, signal
pid=os.fork()
if pid == 0:
 os.setsid(); n=os.open('/dev/null',os.O_RDWR); os.dup2(n,1); os.dup2(n,2)
 os.closerange(3,1024)
 with open('child.pending','w') as f: f.write(str(os.getpid()))
 os.rename('child.pending','child')
 while True: signal.pause()
while not os.path.exists('child'): pass
os._exit(0)
"#,
        ])
        .current_dir(&directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = OwnedChild::spawn(&mut command, "closed-descriptor hook child");
    let deadline = Instant::now() + Duration::from_secs(60);
    while !directory.join("child").is_file() || !child.root_exited().unwrap() {
        assert!(
            Instant::now() < deadline,
            "closed-fd hook child was not ready"
        );
        thread::yield_now();
    }
    let pid: u32 = fs::read_to_string(directory.join("child"))
        .unwrap()
        .parse()
        .unwrap();
    let identity = ProcessIdentity::capture(pid).unwrap();
    struct Guard(ProcessIdentity);
    impl Drop for Guard {
        fn drop(&mut self) {
            if ProcessIdentity::capture(self.0.pid).ok().as_ref() == Some(&self.0) {
                unsafe { libc::kill(self.0.pid as i32, libc::SIGKILL) };
            }
        }
    }
    let _guard = Guard(identity.clone());
    assert!(
        child.observe().unwrap(),
        "closed-fd child was not registered"
    );
    let start = Instant::now();
    let calls = std::cell::Cell::new(0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        child.wait_with_clock(|| {
            let call = calls.get();
            calls.set(call + 1);
            if call == 0 {
                start
            } else {
                start + Duration::from_secs(121)
            }
        });
    }));
    assert!(result.is_err(), "the injected watchdog must fail the wait");
    drop(child);
    assert_ne!(ProcessIdentity::capture(pid).ok().as_ref(), Some(&identity));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn hook_waits_for_a_closed_descriptor_child_to_finish_successfully() {
    use std::{ffi::CString, fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt};
    let directory = std::env::temp_dir().join(format!(
        "arc-hook-closed-fds-success-{}",
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
pid=os.fork()
if pid == 0:
 os.setsid(); n=os.open('/dev/null',os.O_RDWR); os.dup2(n,1); os.dup2(n,2)
 os.closerange(3,1024); gate=os.open('gate',os.O_RDWR)
 with open('child.pending','w') as f: f.write(str(os.getpid()))
 os.rename('child.pending','child')
 os.read(gate,1); os._exit(0)
while not os.path.exists('child'): pass
os._exit(0)
"#,
        ])
        .current_dir(&directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = OwnedChild::spawn(&mut command, "closed-descriptor successful hook child");
    let deadline = Instant::now() + Duration::from_secs(60);
    while !directory.join("child").is_file() || !child.root_exited().unwrap() {
        assert!(
            Instant::now() < deadline,
            "closed-fd hook child was not ready"
        );
        thread::yield_now();
    }
    let pid: u32 = fs::read_to_string(directory.join("child"))
        .unwrap()
        .parse()
        .unwrap();
    let identity = ProcessIdentity::capture(pid).unwrap();
    struct Guard(ProcessIdentity);
    impl Drop for Guard {
        fn drop(&mut self) {
            if ProcessIdentity::capture(self.0.pid).ok().as_ref() == Some(&self.0) {
                unsafe { libc::kill(self.0.pid as i32, libc::SIGKILL) };
            }
        }
    }
    let _guard = Guard(identity.clone());
    assert!(
        child.observe().unwrap(),
        "closed-fd child was not registered"
    );
    let mut gate = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(directory.join("gate"))
        .unwrap();
    gate.write_all(b"x").unwrap();
    assert!(child.wait().success());
    assert_ne!(ProcessIdentity::capture(pid).ok().as_ref(), Some(&identity));
    drop(child);
    fs::remove_dir_all(directory).unwrap();
}
