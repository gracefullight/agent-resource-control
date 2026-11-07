use super::*;

#[test]
fn fixture_coordination_is_shared_across_processes() {
    let mut serial = support::serial();
    let fixture = Fixture::new();
    let mut child = Daemon(
        Command::new(env::current_exe().unwrap())
            .args(["--exact", "fixture_serialization_probe"])
            .env("ARC_FIXTURE_COORDINATION_PROBE", &fixture.directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + READY_TIMEOUT;
    while !fixture.directory.join("ready").exists() {
        assert!(
            child.try_wait().unwrap().is_none(),
            "coordination child exited before waiting"
        );
        assert!(
            Instant::now() < deadline,
            "coordination child did not start"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!fixture.directory.join("acquired").exists());
    serial.release_process_lock();
    assert!(wait_for_exit(&mut child).success());
    assert!(fixture.directory.join("acquired").exists());
}
