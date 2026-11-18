use super::*;

#[test]
fn swift_direct_cleanup_reports_failed_observation_and_live_descendant() {
    let identity = ProcessIdentity::capture(std::process::id()).unwrap();
    let error = wait_for_owned_exit(
        std::slice::from_ref(&identity),
        Duration::from_secs(60),
        |_| Err(io::Error::other("injected identity lookup failure")),
    )
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("injected identity lookup failure"));
    let mut sibling = identity.clone();
    sibling.pid += 1;
    let mut sibling_observations = 0;
    let error = wait_for_owned_exit(
        &[identity.clone(), sibling.clone()],
        Duration::from_secs(60),
        |pid| {
            if pid == identity.pid {
                return Err(io::Error::other("first descendant observation failed"));
            }
            sibling_observations += 1;
            Ok((sibling_observations == 1).then_some(sibling.clone()))
        },
    )
    .unwrap_err();
    assert_eq!(sibling_observations, 2);
    assert!(error
        .to_string()
        .contains("first descendant observation failed"));
    let error = wait_for_owned_exit(std::slice::from_ref(&identity), Duration::ZERO, |_| {
        Ok(Some(identity.clone()))
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(error.to_string().contains(&identity.pid.to_string()));
}

#[test]
fn swift_direct_missing_exit_event_waits_for_the_owned_child_state() {
    let calls = std::cell::Cell::new(0);
    recover_process_registration(
        io::Error::from_raw_os_error(libc::ESRCH),
        Duration::from_secs(60),
        || {
            calls.set(calls.get() + 1);
            Ok(calls.get() == 2)
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 2);
    let error = recover_process_registration(
        io::Error::from_raw_os_error(libc::ESRCH),
        Duration::ZERO,
        || Ok(false),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(error.to_string().contains("never exposed exit"));
    let error = recover_process_registration(
        io::Error::from_raw_os_error(libc::ESRCH),
        Duration::from_secs(60),
        || Err(io::Error::other("owned peer failed")),
    )
    .unwrap_err();
    assert!(error.to_string().contains("owned peer failed"));
    let error = recover_process_registration(
        io::Error::from_raw_os_error(libc::EACCES),
        Duration::from_secs(60),
        || panic!("registration failure must not be hidden"),
    )
    .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(libc::EACCES));
}
