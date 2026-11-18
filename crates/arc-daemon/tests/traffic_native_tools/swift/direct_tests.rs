use super::*;

struct Fixture(std::path::PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "arc-swift-direct-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn child(&self) -> TreeChild {
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                "/usr/bin/head -c 262144 /dev/zero; printf 'stdout-tail'; \
                 /usr/bin/head -c 262144 /dev/zero >&2; printf 'stderr-tail' >&2; \
                 printf ready > ready; IFS= read -r release",
            ])
            .current_dir(&self.0)
            .stdin(Stdio::piped());
        capture_to_project(&self.0, &mut command);
        TreeChild::spawn(command).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn swift_direct_large_output_does_not_block_before_readiness() {
    let fixture = Fixture::new();
    let mut child = fixture.child();
    let end = Instant::now() + Duration::from_secs(60);
    wait_for_gate(
        &mut child,
        &fixture.0,
        |_| fixture.0.join("ready").is_file().then_some(()),
        || Instant::now() >= end,
    )
    .unwrap();
    assert!(fs::metadata(fixture.0.join("direct-stdout")).unwrap().len() > 262144);
    assert!(fs::metadata(fixture.0.join("direct-stderr")).unwrap().len() > 262144);
    child
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"release\n")
        .unwrap();
    assert!(child
        .wait_with_output(Duration::from_secs(60))
        .unwrap()
        .status
        .success());
}

#[test]
fn swift_direct_watchdog_reports_both_streams_and_child_identity() {
    let fixture = Fixture::new();
    let mut child = fixture.child();
    let pid = child.child.id();
    let end = Instant::now() + Duration::from_secs(60);
    // Inject expiry only after the fake child proves it has written both
    // streams; this exercises timeout cleanup without a timed sleep.
    let error = wait_for_gate(
        &mut child,
        &fixture.0,
        |_| None::<()>,
        || fixture.0.join("ready").is_file() || Instant::now() >= end,
    )
    .unwrap_err();
    assert!(error.contains(&format!("pid={pid}")), "{error}");
    assert!(error.contains("watchdog"), "{error}");
    assert!(error.contains("stdout-tail"), "{error}");
    assert!(error.contains("stderr-tail"), "{error}");
    assert!(child.has_exited().is_err() || child.child.try_wait().unwrap().is_some());
}
