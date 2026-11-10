use super::*;
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use arc_ipc::{
    jobs::JobId,
    traffic::{JobRequest, StatusResult},
};

struct ReleaseOnDrop {
    project: PathBuf,
    child: Option<ProcessIdentity>,
}
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        let _ = fs::write(self.project.join("release"), "release");
        let child = self.child.clone().or_else(|| {
            fs::read_to_string(self.project.join("cargo-launches"))
                .ok()?
                .lines()
                .last()?
                .parse::<u32>()
                .ok()
                .and_then(|pid| ProcessIdentity::capture(pid).ok())
        });
        if let Some(child) = &child {
            let deadline = Instant::now() + Duration::from_secs(5);
            while probe_live(child) == IdentityStatus::VerifiedMatch && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            if probe_live(child) == IdentityStatus::VerifiedMatch {
                unsafe { libc::kill(child.pid as i32, libc::SIGKILL) };
            }
        }
    }
}

fn blocked_attachment(f: &Fixture) {
    database(f)
        .execute_batch(
            "CREATE TRIGGER fixture_hold_attachment BEFORE UPDATE OF child_pid ON job_lifecycle
         WHEN NEW.child_pid IS NOT NULL AND OLD.child_pid IS NULL
         BEGIN SELECT RAISE(ABORT, 'fixture holds attachment'); END;",
        )
        .unwrap();
}

fn setup(f: &Fixture, wrapper: &mut OwnedChild, project: &Path) -> (JobId, ProcessIdentity) {
    eventually(|| {
        project.join("started").exists()
            && entry(f, wrapper.child.id())
                .is_some_and(|state| state.0 == "RESERVED" && state.1.is_none())
    });
    let id = JobId::try_from(
        database(f)
            .query_row(
                "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1",
                [wrapper.child.id()],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
    )
    .unwrap();
    let witness = f.root.join(format!(".traffic-witness-{}", id.as_str()));
    let bytes = fs::read(witness).unwrap();
    assert_eq!(bytes.len(), 136);
    assert_eq!(&bytes[..8], b"ARCWIT03");
    let pid = u64::from_be_bytes(bytes[24..32].try_into().unwrap()) as u32;
    let child = ProcessIdentity::capture(pid).unwrap();
    assert_eq!(
        child.start_token,
        u64::from_be_bytes(bytes[32..40].try_into().unwrap()).to_string()
    );
    assert_eq!(cargo_launches(project), vec![pid]);
    assert_eq!(active(f).1, 1);
    (id, child)
}

fn status(f: &Fixture, id: &JobId) -> StatusResult {
    serde_json::from_value(
        traffic(f, &JobRequest::StatusV2 { job_id: id.clone() })
            .result
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn daemon_loss_before_attachment_keeps_a_surviving_child_and_releases_after_exit() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let projects = projects(&f.root, "unattached_live");
    let mut release_on_drop = ReleaseOnDrop {
        project: projects[0].clone(),
        child: None,
    };
    install_cargo_observer(&f);
    blocked_attachment(&f);
    let mut wrapper = spawn_observed_build(&f, &projects[0], true);
    let (id, child) = setup(&f, &mut wrapper, &projects[0]);
    release_on_drop.child = Some(child.clone());
    daemon.stop(libc::SIGTERM);
    database(&f)
        .execute_batch("DROP TRIGGER fixture_hold_attachment")
        .unwrap();
    wrapper.child.kill().unwrap();
    wrapper.child.wait().unwrap();
    assert_eq!(probe_live(&child), IdentityStatus::VerifiedMatch);
    let witness = f.root.join(format!(".traffic-witness-{}", id.as_str()));
    let valid = fs::read(&witness).unwrap();
    let mut forged = valid.clone();
    forged[40] = if forged[40] == b'a' { b'b' } else { b'a' };
    fs::write(&witness, &forged).unwrap();
    fs::rename(
        f.root.join("runtime.json"),
        f.root.join("prior-runtime.json"),
    )
    .unwrap();
    let mut restarted = f.spawn("test");
    assert!(matches!(status(&f, &id), StatusResult::Unconfirmed {}));
    assert_eq!(active(&f).1, 1);
    assert!(entry(&f, wrapper.child.id()).unwrap().1.is_none());
    fs::write(&witness, valid).unwrap();
    assert!(matches!(status(&f, &id), StatusResult::Running { .. }));
    assert_eq!(active(&f).1, 1);
    assert_eq!(entry(&f, wrapper.child.id()).unwrap().1, Some(child.pid));
    fs::write(projects[0].join("release"), "release").unwrap();
    eventually(|| {
        matches!(
            probe_live(&child),
            IdentityStatus::Missing | IdentityStatus::Mismatch
        )
    });
    eventually(|| matches!(status(&f, &id), StatusResult::Abandoned {}));
    assert_eq!(active(&f).1, 0);
    assert_eq!(cargo_launches(&projects[0]), vec![child.pid]);
    assert_eq!(invocations(&projects[0]), 1);
    restarted.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn daemon_loss_before_attachment_recovers_an_already_exited_child_without_replay() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let projects = projects(&f.root, "unattached_exited");
    let mut release_on_drop = ReleaseOnDrop {
        project: projects[0].clone(),
        child: None,
    };
    install_cargo_observer(&f);
    blocked_attachment(&f);
    let mut wrapper = spawn_observed_build(&f, &projects[0], true);
    let (id, child) = setup(&f, &mut wrapper, &projects[0]);
    release_on_drop.child = Some(child.clone());
    daemon.stop(libc::SIGTERM);
    database(&f)
        .execute_batch("DROP TRIGGER fixture_hold_attachment")
        .unwrap();
    fs::write(projects[0].join("release"), "release").unwrap();
    eventually(|| wrapper.child.try_wait().unwrap().is_some());
    assert!(wrapper.child.wait().unwrap().success());
    assert!(matches!(
        probe_live(&child),
        IdentityStatus::Missing | IdentityStatus::Mismatch
    ));
    fs::rename(
        f.root.join("runtime.json"),
        f.root.join("prior-runtime.json"),
    )
    .unwrap();
    let mut restarted = f.spawn("test");
    assert!(matches!(status(&f, &id), StatusResult::Abandoned {}));
    assert_eq!(active(&f).1, 0);
    assert_eq!(cargo_launches(&projects[0]), vec![child.pid]);
    assert_eq!(invocations(&projects[0]), 1);
    restarted.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn witness_creation_failure_reports_spawn_failure_and_runs_no_child() {
    use std::os::unix::fs::PermissionsExt;
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    symlink(binary("arc-exec-fixture"), f.root.join("cargo")).unwrap();
    let marker = f.root.join("witness-failure-marker");
    fs::set_permissions(&f.root, fs::Permissions::from_mode(0o500)).unwrap();
    let output = Command::new(binary("arcctl"))
        .args(["exec", "--", "cargo", "build"])
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env("PATH", &f.root)
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", &marker)
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .output()
        .unwrap();
    fs::set_permissions(&f.root, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(!output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPAWN_FAILED"));
    assert!(!marker.exists());
    assert_eq!(active(&f).0, 0);
    assert_eq!(active(&f).1, 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn restart_retains_unknown_child_until_configured_grace_then_logs_release() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let projects = projects(&f.root, "unknown_grace");
    install_cargo_observer(&f);
    blocked_attachment(&f);
    let mut wrapper = spawn_observed_build(&f, &projects[0], true);
    let (id, child) = setup(&f, &mut wrapper, &projects[0]);
    daemon.stop(libc::SIGTERM);
    database(&f)
        .execute_batch("DROP TRIGGER fixture_hold_attachment")
        .unwrap();
    wrapper.child.kill().unwrap();
    wrapper.child.wait().unwrap();
    fs::write(projects[0].join("release"), "release").unwrap();
    eventually(|| {
        matches!(
            probe_live(&child),
            IdentityStatus::Missing | IdentityStatus::Mismatch
        )
    });
    fs::write(
        f.root.join(format!(".traffic-witness-{}", id.as_str())),
        b"incomplete",
    )
    .unwrap();
    fs::rename(
        f.root.join("runtime.json"),
        f.root.join("prior-runtime.json"),
    )
    .unwrap();
    let mut command = f.command();
    command
        .env("ARC_IPC_PROFILE", "test")
        .env("ARC_TRAFFIC_RECOVERY_GRACE_MS", "500");
    let mut restarted = OwnedChild::new(command.spawn().unwrap());
    f.wait_for(&mut restarted, "arc-daemon ready.");
    assert!(matches!(status(&f, &id), StatusResult::Unconfirmed {}));
    assert_eq!(active(&f).1, 1);
    eventually(|| matches!(status(&f, &id), StatusResult::Abandoned {}));
    assert_eq!(active(&f).1, 0);
    let events: i64 = database(&f).query_row("SELECT count(*) FROM audit_events WHERE entity_id=?1 AND event_type IN ('TRAFFIC_UNKNOWN_OWNER','TRAFFIC_GRACE_EXPIRED')", [id.as_str()], |row| row.get(0)).unwrap();
    assert_eq!(events, 2);
    assert_eq!(cargo_launches(&projects[0]), vec![child.pid]);
    assert_eq!(invocations(&projects[0]), 1);
    restarted.stop(libc::SIGTERM);
    f.report();
}
