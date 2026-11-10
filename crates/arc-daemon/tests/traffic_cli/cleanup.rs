use super::*;
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use std::os::fd::AsRawFd;

fn terminated(identity: &ProcessIdentity) -> bool {
    matches!(
        probe_live(identity),
        IdentityStatus::Missing | IdentityStatus::Mismatch
    )
}

// This independent guard keeps the regression's observed processes owned even
// if an assertion fails while exercising BuildChild's cleanup.
struct IdentityCleanup(Vec<ProcessIdentity>);
impl Drop for IdentityCleanup {
    fn drop(&mut self) {
        let mut errors = Vec::new();
        for identity in &self.0 {
            errors.extend(process_cleanup::kill_tree(identity, 0));
        }
        process_cleanup::report_errors(errors);
    }
}

#[test]
fn dropping_fixture_reaps_wrapper_and_stops_cargo_and_build_script() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let project = projects(&f.root, "cleanup");
    install_cargo_observer(&f);
    let wrapper = spawn_observed_build(&f, &project[0], true);
    let owner = ProcessIdentity::capture(wrapper.child.id()).unwrap();
    eventually(|| project[0].join("started").exists());
    let cargo = ProcessIdentity::capture(cargo_launches(&project[0])[0]).unwrap();
    let descendants: Vec<_> = children(cargo.pid)
        .into_iter()
        .filter_map(|pid| process_cleanup::capture(pid).unwrap_or_else(|error| panic!("{error}")))
        .collect();
    assert!(
        !descendants.is_empty(),
        "the blocking build script must be running"
    );
    assert_eq!(probe_live(&cargo), IdentityStatus::VerifiedMatch);
    drop(wrapper);
    eventually(|| terminated(&owner) && terminated(&cargo) && descendants.iter().all(terminated));
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn interrupted_cached_launch_reaps_verified_cargo_tree() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let project = projects(&f.root, "cached_interrupt")[0].clone();
    fs::remove_file(project.join("build.rs")).unwrap();
    install_cargo_observer(&f);
    let mut warm = recovery::spawn_cached_build(&f, &project, &project.join("warm.stderr"));
    finish(&mut warm);
    drop(warm);
    assert_eq!(cargo_launches(&project).len(), 1);

    // Keep the second, cached Cargo process alive on Cargo's own build lock.
    let lock = File::options()
        .read(true)
        .write(true)
        .open(project.join("target/debug/.cargo-lock"))
        .unwrap();
    assert_eq!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0,
        "could not hold the fixture's build-directory lock"
    );
    // The handshake lets the test record identities before the shim execs
    // Cargo. Its long-lived child proves cleanup includes observed descendants.
    fs::write(
        f.root.join("cargo"),
        "#!/bin/sh\n/bin/sleep 120 &\nprintf '%s\\n' \"$!\" > \"${TRAFFIC_LAUNCH_LOG}.descendant\" || exit 125\nprintf '%s\\n' \"$$\" >> \"$TRAFFIC_LAUNCH_LOG\" || exit 125\nwhile [ ! -f \"${TRAFFIC_LAUNCH_LOG}.allow\" ]; do /bin/sleep 0.01; done\nexec \"$TRAFFIC_REAL_CARGO\" \"$@\"\n",
    )
    .unwrap();
    let mut safety = IdentityCleanup(Vec::new());
    let mut interrupted = false;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let wrapper = recovery::spawn_cached_build(&f, &project, &project.join("cached.stderr"));
        let owner = ProcessIdentity::capture(wrapper.child.id()).unwrap();
        safety.0.push(owner.clone());
        eventually(|| cargo_launches(&project).len() == 2);
        let pid = cargo_launches(&project)[1];
        assert!(children(owner.pid).contains(&pid));
        let cargo = ProcessIdentity::capture(pid).unwrap();
        safety.0.push(cargo.clone());
        let descendant_pid: u32 = fs::read_to_string(project.join("cargo-launches.descendant"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(children(cargo.pid).contains(&descendant_pid));
        let descendant = ProcessIdentity::capture(descendant_pid).unwrap();
        safety.0.push(descendant);
        fs::write(project.join("cargo-launches.allow"), "continue").unwrap();
        eventually(|| cargo_pid(cargo.pid));
        assert!(safety
            .0
            .iter()
            .all(|identity| probe_live(identity) == IdentityStatus::VerifiedMatch));
        for pid in children(cargo.pid) {
            if let Some(identity) =
                process_cleanup::capture(pid).unwrap_or_else(|error| panic!("{error}"))
            {
                if !safety.0.contains(&identity) {
                    safety.0.push(identity);
                }
            }
        }
        interrupted = true;
        panic!("simulated interruption of a cached Cargo launch");
    }));
    assert!(
        result.is_err() && interrupted,
        "fixture did not reach interruption"
    );
    let errors = process_cleanup::take_unwind_errors();
    assert!(
        errors.is_empty(),
        "cleanup failed during unwind: {errors:?}"
    );
    eventually(|| safety.0.iter().all(terminated));
    println!(
        "cached interruption: wrapper, Cargo, and {} observed descendants terminated",
        safety.0.len() - 2
    );
    drop(lock);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn identity_capture_observes_live_process() {
    let pid = std::process::id();
    let identity = process_cleanup::capture(pid).unwrap().unwrap();
    assert_eq!(identity.pid, pid);
    assert_eq!(probe_live(&identity), IdentityStatus::VerifiedMatch);
}

#[test]
fn identity_capture_distinguishes_verified_disappearance() {
    let result = process_cleanup::capture_with(
        123,
        |_| Err(std::io::Error::from_raw_os_error(libc::ESRCH)),
        |_| Ok(true),
    );
    assert_eq!(result.unwrap(), None);
}

#[test]
fn identity_capture_reports_unavailable_observation() {
    let error = process_cleanup::capture_with(
        123,
        |_| Err(std::io::Error::from_raw_os_error(libc::EACCES)),
        |_| Err(std::io::Error::from_raw_os_error(libc::EPERM)),
    )
    .unwrap_err();
    let message = error.to_string();
    assert!(message.contains("pid 123"), "{message}");
    assert!(message.contains("os error 13"), "{message}");
    assert!(message.contains("os error 1"), "{message}");
}

#[test]
fn descendant_enumeration_reports_native_failure() {
    let error = process_cleanup::children_with(123, |_| {
        Err(std::io::Error::from_raw_os_error(libc::EPERM))
    })
    .unwrap_err();
    assert!(error.to_string().contains("pid 123"));
    assert!(error.to_string().contains("os error 1"));
}

#[test]
fn cleanup_observation_error_during_unwind_is_recorded_without_a_second_panic() {
    struct FailingCleanup;
    impl Drop for FailingCleanup {
        fn drop(&mut self) {
            process_cleanup::report_errors(vec!["permission denied during observation".into()]);
        }
    }
    let outcome = std::panic::catch_unwind(|| {
        let _guard = FailingCleanup;
        panic!("simulated interruption");
    });
    assert!(outcome.is_err());
    let errors = process_cleanup::take_unwind_errors();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("permission denied during observation"));
}
