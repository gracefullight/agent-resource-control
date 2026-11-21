use super::*;

#[test]
fn pnpm_manifest_change_after_capture_is_observed_on_both_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    symlink(cli::binary("arc-exec-fixture"), s.bin.join("pnpm")).unwrap();
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    let manifest = s.repo.join("package.json");
    let literal = json!({"scripts":{"deploy":"cargo build && gh workflow run ci.yml"}}).to_string();
    write(&manifest, &literal);
    let env = [(
        "ARC_TEST_CAPTURE_REPLACE_PACKAGE",
        Some(manifest.to_str().unwrap()),
    )];
    let mut daemon = f.spawn_isolated("test");
    let argv = ["pnpm", "run", "deploy"];
    let inspection = s.classify(&f, &argv, &env);
    assert_eq!(inspection["classification"], "UNKNOWN");
    assert_eq!(inspection["uncertainty"], "OBSERVE");
    assert_ne!(fs::read_to_string(&manifest).unwrap(), literal);
    write(&manifest, literal);
    let (job, _) = s.exec(&f, &argv, &env);
    assert_eq!(job["classification"]["classification"], "UNKNOWN");
    assert_eq!(job["classification"]["uncertainty"], "OBSERVE");
    assert_recorded_fixture_child(&s, &job);
    assert_eq!(admission_counts(&f), (0, 0));
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn same_pc_runner_starts_a_recorded_locally_admitted_build_child() {
    use std::os::unix::fs::PermissionsExt;
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    symlink(cli::binary("arc-exec-fixture"), s.bin.join("cargo")).unwrap();
    let runner = s.repo.join("fixture-runner");
    write(
        &runner,
        "#!/bin/sh\nexec \"$ARC_RUNNER_ARCCTL\" exec -- cargo build\n",
    );
    fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
    let mut daemon = f.spawn_isolated("test");
    let inspection = s.classify(&f, &["cargo", "build"], &[]);
    assert_eq!(inspection["classification"], "BUILD");
    assert_eq!(inspection["locality"], "HOST_LOCAL");
    let before = admission_counts(&f);
    let template = s.command(&f, &[]);
    let mut command = Command::new(&runner);
    command
        .env_clear()
        .current_dir(&s.repo)
        .stdin(Stdio::null());
    for (key, value) in template.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        }
    }
    command.env("ARC_RUNNER_ARCCTL", cli::binary("arcctl"));
    command.env("ARC_FIXTURE_MODE", "success");
    let output = child_watchdog::output(&mut command, "same-PC runner local workload").unwrap();
    assert!(output.status.success(), "{output:?}");
    let id = cli::recent(&f);
    let response = protocol::call(
        &f,
        OPERATION_JOB_STATUS_LOCALITY,
        json!(StatusRequest::ById { job_id: id }),
    );
    let job = response.result.unwrap()["job"].clone();
    assert_eq!(job["classification"]["classification"], "BUILD");
    assert_eq!(job["classification"]["locality"], "HOST_LOCAL");
    assert_eq!(job["outcome"]["code"], 0);
    assert_recorded_fixture_child(&s, &job);
    assert_eq!(s.calls(), 1);
    let after = admission_counts(&f);
    assert_eq!((after.0 - before.0, after.1 - before.1), (1, 1));
    daemon.stop(libc::SIGTERM);
    f.report();
}
