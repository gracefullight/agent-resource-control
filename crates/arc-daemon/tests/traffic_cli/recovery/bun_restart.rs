use super::*;

#[test]
fn real_bun_queued_and_running_jobs_survive_daemon_restart() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let (first_path, mut first) = status_compat::real_bun_fixture(&f, "bun_restart_first");
    eventually(|| {
        first_path.join("started").exists()
            && entry(&f, first.child.id()).is_some_and(|e| e.0 == "RUNNING")
    });
    let (second_path, mut second) = status_compat::real_bun_fixture(&f, "bun_restart_second");
    eventually(|| {
        entry(&f, second.child.id()).is_some_and(|e| e.0 == "QUEUED")
            && fs::read_to_string(second_path.join("stderr"))
                .is_ok_and(|stderr| stderr.contains("QUEUED"))
    });
    assert!(!second_path.join("started").exists());
    assert!(fs::read_to_string(second_path.join("stderr"))
        .unwrap()
        .contains("QUEUED"));
    assert_eq!(
        unsafe { libc::kill(second.child.id() as i32, libc::SIGSTOP) },
        0
    );
    daemon.stop(libc::SIGTERM);
    fs::rename(
        f.root.join("runtime.json"),
        f.root.join("prior-runtime.json"),
    )
    .unwrap();
    let mut restarted = f.spawn("test");
    assert_eq!(active(&f).1, 1);
    assert!(entry(&f, second.child.id()).is_some_and(|e| e.0 == "QUEUED"));
    assert_eq!(
        unsafe { libc::kill(second.child.id() as i32, libc::SIGCONT) },
        0
    );
    fs::write(first_path.join("release"), "release").unwrap();
    finish(&mut first);
    eventually(|| {
        second_path.join("started").exists()
            && entry(&f, second.child.id()).is_some_and(|e| e.0 == "RUNNING")
    });
    fs::write(second_path.join("release"), "release").unwrap();
    finish(&mut second);
    assert_eq!(invocations(&first_path), 1);
    assert_eq!(invocations(&second_path), 1);
    assert_eq!(active(&f).1, 0);
    assert_eq!(entry(&f, first.child.id()).unwrap().0, "COMPLETED");
    assert_eq!(entry(&f, second.child.id()).unwrap().0, "COMPLETED");
    restarted.stop(libc::SIGTERM);
    f.report();
}
