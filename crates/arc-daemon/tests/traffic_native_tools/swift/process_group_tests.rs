//! Exercise an exited group leader and an unavailable member observation.
use super::*;
use std::{
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    process::Stdio,
};

fn wait_for(description: &str, mut observe: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(60);
    while !observe() {
        assert!(Instant::now() < end, "never observed {description}");
        std::thread::yield_now();
    }
}

// If an assertion fails after the leader exits, the test still owns both
// verified identities and terminates them before returning to the test runner.
struct SiblingGuard(Vec<ProcessIdentity>);
impl Drop for SiblingGuard {
    fn drop(&mut self) {
        for identity in &self.0 {
            let _ = signal(identity, libc::SIGKILL);
        }
        let _ = wait_for_owned_exit(&self.0, Duration::from_secs(60), crate::cleanup::capture);
    }
}

fn fifo(directory: &std::path::Path, name: &str) -> PathBuf {
    let path = directory.join(name);
    let raw = CString::new(path.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o600) }, 0);
    path
}

#[test]
fn swift_group_lookup_observes_leader_exit_after_the_initial_check() {
    let mut observations = 0;
    events::signal_owned_group(
        42,
        || {
            observations += 1;
            Ok(observations == 2)
        },
        || Err(io::Error::from_raw_os_error(libc::ESRCH)),
        || panic!("an unobserved process group must never be signalled"),
    )
    .expect("a leader that exits during lookup is a completed cleanup transition");
    assert_eq!(observations, 2);
}

#[test]
fn swift_group_signal_observes_leader_exit_and_preserves_real_errors() {
    let mut observations = 0;
    events::signal_owned_group(
        42,
        || {
            observations += 1;
            Ok(observations == 2)
        },
        || Ok(42),
        || Err(io::Error::from_raw_os_error(libc::ESRCH)),
    )
    .expect("a leader that exits while signalling is a completed cleanup transition");
    assert_eq!(observations, 2);

    let error = events::signal_owned_group(
        42,
        || Ok(false),
        || Err(io::Error::from_raw_os_error(libc::EACCES)),
        || panic!("an unobserved process group must never be signalled"),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error:?}");
    assert!(error.to_string().contains("process group"));

    let error = events::signal_owned_group(
        42,
        || Ok(false),
        || Ok(42),
        || Err(io::Error::from_raw_os_error(libc::EACCES)),
    )
    .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(libc::EACCES));

    let error = events::signal_owned_group(
        42,
        || Ok(false),
        || Ok(43),
        || panic!("a different process group must never be signalled"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("current group 43"));
}

#[test]
fn exited_leader_cleanup_continues_after_group_capture_failure() {
    let directory = crate::suite::root().join(format!(
        "target/traffic-native-tools/swift-group-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir_all(&directory).unwrap();
    let fifos = [fifo(&directory, "first"), fifo(&directory, "second")];
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            r#"
/bin/cat "$ARC_FIXTURE_DIR/first" & first=$!
/bin/cat "$ARC_FIXTURE_DIR/second" & second=$!
printf '%s %s\n' "$first" "$second" > "$ARC_FIXTURE_DIR/pending"
/bin/mv "$ARC_FIXTURE_DIR/pending" "$ARC_FIXTURE_DIR/ready"
IFS= read -r release
exit 0
"#,
        ])
        .env_clear()
        .env("ARC_FIXTURE_DIR", &directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut runner = TreeChild::spawn(command).unwrap();
    let ready = directory.join("ready");
    wait_for("both siblings to publish their PIDs", || {
        assert!(
            !runner.has_exited().unwrap(),
            "leader exited before readiness"
        );
        ready.is_file()
    });
    let pids: Vec<u32> = fs::read_to_string(ready)
        .unwrap()
        .split_whitespace()
        .map(|part| part.parse().unwrap())
        .collect();
    assert_eq!(pids.len(), 2);
    let identities: Vec<_> = pids
        .iter()
        .map(|pid| {
            assert_eq!(
                unsafe { libc::getpgid(*pid as i32) },
                runner.root.pid as i32
            );
            crate::cleanup::child_identity(runner.root.pid, *pid)
                .unwrap()
                .expect("published sibling must be the leader's child")
        })
        .collect();
    let _guard = SiblingGuard(identities.clone());
    let mut gates: Vec<Option<File>> = vec![None, None];
    wait_for("both siblings to open their FIFOs", || {
        assert!(
            !runner.has_exited().unwrap(),
            "leader exited before FIFO readiness"
        );
        for (index, path) in fifos.iter().enumerate() {
            if gates[index].is_none() {
                match OpenOptions::new()
                    .write(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(path)
                {
                    Ok(file) => gates[index] = Some(file),
                    Err(error) if error.raw_os_error() == Some(libc::ENXIO) => (),
                    Err(error) => panic!("open sibling FIFO: {error}"),
                }
            }
            assert_eq!(
                crate::cleanup::capture(identities[index].pid)
                    .unwrap()
                    .as_ref(),
                Some(&identities[index]),
                "sibling exited before FIFO readiness"
            );
        }
        gates.iter().all(Option::is_some)
    });
    runner
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"exit\n")
        .unwrap();
    wait_for("leader exit without reaping", || {
        runner.has_exited().unwrap()
    });
    for identity in &identities {
        assert_eq!(
            crate::cleanup::capture(identity.pid).unwrap().as_ref(),
            Some(identity),
            "sibling must outlive the leader"
        );
    }

    let mut seen = Vec::new();
    let mut failed_pid = None;
    let error = runner
        .cleanup_with_observers(
            |child| child.signal_group(libc::SIGSTOP),
            &mut |_| Ok(()),
            &mut |pid| {
                seen.push(pid);
                if failed_pid.is_none() {
                    failed_pid = Some(pid);
                    Err(io::Error::other("injected group capture failure"))
                } else {
                    crate::cleanup::capture(pid)
                }
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("injected group capture failure"));
    assert_eq!(seen.len(), 2, "both enumerated siblings must be visited");
    assert!(identities
        .iter()
        .all(|identity| seen.contains(&identity.pid)));
    let failed_pid = failed_pid.unwrap();
    let later = identities.iter().find(|id| id.pid != failed_pid).unwrap();
    assert!(
        runner.owned.contains(later),
        "later sibling was not captured"
    );
    assert_ne!(
        crate::cleanup::capture(later.pid).unwrap().as_ref(),
        Some(later),
        "later sibling survived cleanup"
    );

    // The injected failure deliberately made the first identity unavailable to
    // the cleanup pass; the fixture guard still has its verified identity.
    let failed = identities.iter().find(|id| id.pid == failed_pid).unwrap();
    signal(failed, libc::SIGKILL).unwrap();
    wait_for_owned_exit(
        std::slice::from_ref(failed),
        Duration::from_secs(60),
        crate::cleanup::capture,
    )
    .unwrap();
    assert!(runner.cleaned);
    fs::remove_dir_all(directory).unwrap();
}
