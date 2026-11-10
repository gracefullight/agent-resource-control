use super::*;
use std::{
    io::{Read, Write},
    os::unix::process::ExitStatusExt,
};

fn class_execution(subcommand: &str, class: &str, reason: &str) {
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let project = projects(&f.root, subcommand);
    let mut first = spawn_build(&f, &project[0], true);
    eventually(|| project[0].join("started").exists());
    symlink(binary("arc-exec-fixture"), f.root.join("cargo")).unwrap();
    let args = [subcommand, "--offline", "two words", "argv-secret-sentinel"];
    let configure = |command: &mut Command, marker: &Path| {
        command
            .args(args)
            .current_dir(&project[1])
            .env("ARC_FIXTURE_MODE", "echo")
            .env("ARC_FIXTURE_MARKER", marker)
            .env("PRIVATE_ENV", "environment-secret-sentinel")
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    };
    let mut direct = Command::new(f.root.join("cargo"));
    configure(&mut direct, &f.root.join("direct-marker"));
    let mut direct = direct.spawn().unwrap();
    direct
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin\0bytes\n")
        .unwrap();
    let expected = direct.wait_with_output().unwrap();
    assert_eq!(expected.status.code(), Some(17));

    let marker = f.root.join("managed-marker");
    let mut command = Command::new(binary("arcctl"));
    command
        .args(["exec", "--", "cargo"])
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env(
            "PATH",
            format!("{}:{}", f.root.display(), std::env::var("PATH").unwrap()),
        );
    configure(&mut command, &marker);
    let mut wrapper = OwnedChild::new(command.spawn().unwrap());
    eventually(|| entry(&f, wrapper.child.id()).is_some_and(|e| e.0 == "QUEUED"));
    assert!(!marker.exists());
    let (actual_class, actual_reason): (String, String) = database(&f).query_row(
        "SELECT j.job_class,l.classification_reason FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id WHERE l.observer_pid=?1",
        [wrapper.child.id()], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
    assert_eq!(
        (actual_class.as_str(), actual_reason.as_str()),
        (class, reason)
    );
    fs::write(project[0].join("release"), "release").unwrap();
    finish(&mut first);
    eventually(|| {
        marker.exists() && entry(&f, wrapper.child.id()).is_some_and(|e| e.0 == "RUNNING")
    });
    let child_pid = entry(&f, wrapper.child.id()).unwrap().1.unwrap();
    assert_eq!(unsafe { libc::getpgid(child_pid as i32) }, child_pid as i32);
    wrapper
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin\0bytes\n")
        .unwrap();
    eventually(|| wrapper.child.try_wait().unwrap().is_some());
    assert_eq!(wrapper.child.wait().unwrap().code(), Some(17));
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    wrapper
        .child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();
    wrapper
        .child
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut stderr)
        .unwrap();
    assert_eq!(stdout, expected.stdout);
    assert!(stderr.ends_with(&expected.stderr));
    assert!(String::from_utf8_lossy(&stderr).contains("QUEUED\nWaiting to start."));
    assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
    assert_eq!(active(&f).1, 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn scheduled_test_and_lint_preserve_native_streams_and_admission() {
    let _serial = support::serial();
    class_execution("test", "TEST", "CARGO_TEST");
    class_execution("clippy", "LINT", "CARGO_CLIPPY");
}

#[test]
fn scheduled_child_signal_is_returned_as_native_signal_status() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    symlink(binary("arc-exec-fixture"), f.root.join("cargo")).unwrap();
    let output = Command::new(binary("arcctl"))
        .args(["exec", "--", "cargo", "test"])
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env(
            "PATH",
            format!("{}:{}", f.root.display(), std::env::var("PATH").unwrap()),
        )
        .env("ARC_FIXTURE_MODE", "term")
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .output()
        .unwrap();
    assert_eq!(output.status.signal(), Some(libc::SIGTERM));
    let (outcome, signal): (String, i64) = database(&f)
        .query_row(
            "SELECT outcome_kind,signal FROM job_lifecycle WHERE outcome_kind='SIGNALED'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        (outcome.as_str(), signal),
        ("SIGNALED", libc::SIGTERM as i64)
    );
    assert_eq!(active(&f).1, 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}
