use super::*;
use std::os::unix::net::UnixListener;

#[test]
fn gh_and_buildx_same_execution_bypass_held_permit_and_buildx_cli_is_metered() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    super::endpoints::install(&s);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    write(
        &s.home.join(".docker/buildx/current"),
        json!({"Key":"remote","Name":"remote-builder","Global":false}).to_string(),
    );
    let mut daemon = f.spawn("test");
    // A separate real CLI owns the local Build permit. Its offline cargo
    // child reaches a barrier only after admission and native spawn.
    symlink(cli::binary("arc-exec-fixture"), s.bin.join("cargo")).unwrap();
    let listener = UnixListener::bind(f.root.join("build-control")).unwrap();
    let mut local_command = s.command(&f, &[]);
    local_command
        .env("ARC_FIXTURE_CONTROL", f.root.join("build-control"))
        .env("ARC_FIXTURE_MARKER", f.root.join("local-build-call"))
        .args(["exec", "--", "cargo", "build"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut local = super::super::runtime::OwnedChild::spawn_with_descendants(
        local_command,
        f.root.join("build-owners"),
    )
    .unwrap();
    let (mut release, child) = cli::accept(&listener);
    local.own_process(serde_json::from_value(child["identity"].clone()).unwrap());
    let db = rusqlite::Connection::open(&f.database).unwrap();
    let counts = || {
        db.query_row("SELECT (SELECT count(*) FROM traffic_entries), (SELECT count(*) FROM traffic_entries WHERE state='QUEUED'), (SELECT count(*) FROM permits), (SELECT count(*) FROM permits WHERE state='ACTIVE'), (SELECT count(*) FROM jobs WHERE queue_at IS NOT NULL OR reserved_at IS NOT NULL)", [], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?, row.get::<_, i64>(4)?))).unwrap()
    };
    let before = counts();
    assert_eq!((before.1, before.2, before.3), (0, 1, 1));

    let gh = ["gh", "workflow", "run", "build.yml"];
    let inspection = s.classify(&f, &gh, &[]);
    let (job, _) = s.exec(&f, &gh, &[]);
    assert!(!assert_routes("held gh", &inspection, &job, "REMOTE_CI"));
    assert_eq!(job["classification"]["reason"], "GH_WORKFLOW_RUN");
    assert_eq!(s.calls(), 1);
    assert_eq!(counts(), before, "gh added local admission state");

    let buildx = ["docker", "buildx", "build", "."];
    let inspection = s.classify(&f, &buildx, &[]);
    let (job, job_id) = s.exec(&f, &buildx, &[]);
    assert!(!assert_routes(
        "held Buildx",
        &inspection,
        &job,
        "REMOTE_EXEC"
    ));
    assert_eq!(job["classification"]["reason"], "REMOTE_ENDPOINT_EXEC");
    assert_eq!(s.calls(), 2);
    assert_eq!(counts(), before, "Buildx added local admission state");

    // This is the local stub child's native accounting, not remote-builder CPU.
    let mut command = s.command(&f, &[]);
    command.args(["meter", "summary", "--job", job_id.as_str(), "--json"]);
    let output = child_watchdog::output(&mut command, "Buildx local CLI meter").unwrap();
    assert!(output.status.success(), "meter summary: {output:?}");
    let summary: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["job_id"], job_id.as_str());
    assert_eq!(summary["accounting"]["state"], "recorded", "{summary}");
    let report = &summary["accounting"]["report"];
    assert_eq!(report["child"]["pid"], job["child"]["pid"]);
    assert!(report["cpu_core_seconds"]["total"].is_number(), "{summary}");
    assert_eq!(report["scope"], "native_waited_descendants");
    assert_eq!(report["outcome"]["code"], 17);
    drop(db);
    release.write_all(b"x").unwrap();
    let output = local.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(17));
    daemon.stop(libc::SIGTERM);
    f.report();
}
