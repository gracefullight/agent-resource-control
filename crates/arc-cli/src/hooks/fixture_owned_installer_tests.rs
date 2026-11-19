use super::*;
use std::{fs, process::Stdio};

#[test]
fn nested_watchdog_kills_owned_group_and_reaps_root() {
    let directory = std::env::temp_dir().join(format!(
        "arc-nested-watchdog-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&directory).unwrap();
    let mut command = Command::new("/usr/bin/python3");
    command
        .args([
            "-c",
            r#"
import os, signal
helper = os.fork()
if helper == 0:
 os.setsid()
 descendant = os.fork()
 if descendant == 0:
  while True: signal.pause()
 with open('pending','w') as f: f.write(f'{os.getpid()} {descendant}')
 os.rename('pending','ready')
 while True: signal.pause()
os.waitpid(helper, 0)
"#,
        ])
        .current_dir(&directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = OwnedChild::spawn_nested(&mut command, "nested watchdog expiry");
    let root = child.child_mut().id();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !directory.join("ready").is_file() {
        assert!(!child.root_exited().unwrap());
        assert!(
            Instant::now() < deadline,
            "nested helper did not become ready"
        );
        thread::yield_now();
    }
    let pids: Vec<u32> = fs::read_to_string(directory.join("ready"))
        .unwrap()
        .split_whitespace()
        .map(|pid| pid.parse().unwrap())
        .collect();
    assert_eq!(pids.len(), 2);
    let identities: Vec<_> = pids
        .iter()
        .map(|pid| ProcessIdentity::capture(*pid).unwrap())
        .collect();
    struct Guard(Vec<ProcessIdentity>);
    impl Drop for Guard {
        fn drop(&mut self) {
            for identity in &self.0 {
                if ProcessIdentity::capture(identity.pid).ok().as_ref() == Some(identity) {
                    unsafe { libc::kill(identity.pid as i32, libc::SIGKILL) };
                }
            }
        }
    }
    let _guard = Guard(identities.clone());
    assert_eq!(unsafe { libc::getpgid(root as i32) }, root as i32);
    assert_eq!(unsafe { libc::getpgid(pids[0] as i32) }, pids[0] as i32);
    assert_eq!(unsafe { libc::getpgid(pids[1] as i32) }, pids[0] as i32);
    let start = Instant::now();
    let calls = std::cell::Cell::new(0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        child.wait_with_clock(|| {
            let call = calls.get();
            calls.set(call + 1);
            if call == 0 {
                start
            } else {
                start + WATCHDOG + Duration::from_secs(1)
            }
        });
    }));
    assert!(result.is_err(), "injected watchdog did not fire");
    drop(child);
    for identity in &identities {
        assert_ne!(
            ProcessIdentity::capture(identity.pid).ok().as_ref(),
            Some(identity)
        );
    }
    let mut status = 0;
    assert_eq!(
        unsafe { libc::waitpid(root as i32, &mut status, libc::WNOHANG) },
        -1
    );
    assert_eq!(
        io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
    fs::remove_dir_all(directory).unwrap();
}
