use super::*;

#[path = "closing_locality/docker_controls.rs"]
mod docker_controls;
#[path = "closing_locality/integration.rs"]
mod integration;
#[path = "closing_locality/kube_controls.rs"]
mod kube_controls;

const PROD_CONTEXT_HASH: &str = "6754af9632a2745e85c293e5aac0863370d9bd3330b9938c00cadfd215227d77";

pub(super) fn prod_context(s: &Sandbox, host: &str) {
    write(
        &s.home
            .join(".docker/contexts/meta")
            .join(PROD_CONTEXT_HASH)
            .join("meta.json"),
        json!({"Name":"prod","Endpoints":{"docker":{"Host":host}}}).to_string(),
    );
}

pub(super) fn assert_recorded_fixture_child(s: &Sandbox, job: &Value) {
    let marker = fs::read_to_string(&s.marker).unwrap();
    let pid: u64 = marker.lines().last().unwrap().parse().unwrap();
    assert_eq!(job["child"]["pid"], pid, "record must name the stub child");
}

#[test]
fn proven_gh_remote_is_the_same_inspected_and_executed_child() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    let mut daemon = f.spawn_isolated("test");
    let argv = ["gh", "workflow", "run", "build.yml"];
    let inspection = s.classify(&f, &argv, &[]);
    let (job, _) = s.exec(&f, &argv, &[]);
    assert!(!assert_routes("proven gh", &inspection, &job, "REMOTE_CI"));
    assert_eq!(job["classification"]["reason"], "GH_WORKFLOW_RUN");
    assert_eq!(s.calls(), 1);
    assert_recorded_fixture_child(&s, &job);
    assert_eq!(admission_counts(&f), (0, 0));
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn gitlab_explicit_public_endpoint_and_unresolved_controls_on_both_routes() {
    super::git_system::gitlab_explicit_public_endpoint_and_unresolved_controls_on_both_routes();
}

#[test]
fn kubectl_without_placement_proof_stays_observed_without_starting_a_probe() {
    kube_controls::kubectl_without_placement_proof_stays_observed_without_starting_a_probe();
}

#[test]
fn pnpm_literal_leaf_facts_are_captured_without_lending_unproved_facts() {
    use std::{os::unix::net::UnixDatagram, time::Duration};
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    symlink(cli::binary("arc-exec-fixture"), s.bin.join("pnpm")).unwrap();
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    let socket_path = f.root.join("pnpm-capture.sock");
    let socket = UnixDatagram::bind(&socket_path).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let socket_path = socket_path.to_str().unwrap();
    let env = [("ARC_TEST_CAPTURE_SOCKET", Some(socket_path))];
    let receive = |expected_route: &str| -> Value {
        let mut bytes = vec![0; 32 * 1024];
        let length = socket
            .recv(&mut bytes)
            .expect("capture observation watchdog");
        let at = bytes[..length].iter().position(|b| *b == b'\t').unwrap();
        assert_eq!(&bytes[..at], expected_route.as_bytes());
        serde_json::from_slice(&bytes[at + 1..length]).unwrap()
    };
    let has_gh = |context: &Value| {
        context["facts"].as_array().unwrap().iter().any(|fact| {
            fact["kind"] == "CI_HOST"
                && fact["selector"] == "gh"
                && fact["endpoints"] == json!(["https://github.com"])
        })
    };
    let argv = ["pnpm", "run", "deploy"];
    let mut daemon = f.spawn_isolated("test");
    for (body, captured, events) in [
        ("cargo build && gh workflow run ci.yml", true, 3),
        ("cargo build && $DEPLOY_STEP", false, 1),
        ("gh workflow run ci.yml --repo github.com/example/app && gh workflow run ci.yml --repo localhost/example/app", false, 3),
    ] {
        write(&s.repo.join("package.json"), json!({"scripts":{"deploy":body}}).to_string());
        let inspection = s.classify(&f, &argv, &env);
        let mut root = Value::Null;
        for _ in 0..events { root = receive("argv"); }
        let shell = receive("shell");
        assert_eq!(has_gh(&root), captured, "{body}");
        assert_eq!(has_gh(&shell), captured, "{body}");
        let (job, _) = s.exec(&f, &argv, &env);
        for _ in 0..events { root = receive("argv"); }
        assert_eq!(has_gh(&root), captured, "{body}");
        eprintln!("pnpm capture {body}: classify={} exec={}", inspection["classification"], job["classification"]["classification"]);
        // The parser and managed-exec package-script routes belong to the
        // other lanes; this test checks capture rather than their result.
        if captured {
            for result in [&inspection, &job["classification"]] {
                match result["classification"].as_str().unwrap() {
                    "UNKNOWN" => {
                        assert_eq!(result["locality"], "UNKNOWN");
                        assert_eq!(result["uncertainty"], "OBSERVE");
                    }
                    "MIXED" => {
                        assert_eq!(result["locality"], "MIXED");
                        assert_eq!(result["uncertainty"], "NONE");
                    }
                    other => panic!("literal mixed script was classified as {other}: {result}"),
                }
            }
        } else {
            assert!(!assert_routes(body, &inspection, &job, "UNKNOWN"));
            assert_eq!(admission_counts(&f), (0, 0));
        }
        assert_recorded_fixture_child(&s, &job);
    }
    assert_eq!(s.calls(), 3);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn pnpm_deploy_literal_build_and_remote_ci_is_mixed_on_both_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    symlink(cli::binary("arc-exec-fixture"), s.bin.join("pnpm")).unwrap();
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    write(
        &s.repo.join("package.json"),
        json!({"scripts":{"deploy":"cargo build && gh workflow run ci.yml"}}).to_string(),
    );
    let mut daemon = f.spawn_isolated("test");
    let argv = ["pnpm", "run", "deploy"];
    let inspection = s.classify(&f, &argv, &[]);
    let (job, _) = s.exec(&f, &argv, &[]);
    assert_recorded_fixture_child(&s, &job);
    assert_eq!(s.calls(), 1);
    eprintln!(
        "literal pnpm deploy: classify={} exec={}",
        inspection, job["classification"]
    );
    for (route, result) in [("classify", &inspection), ("exec", &job["classification"])] {
        assert_eq!(result["classification"], "MIXED", "{route}: {result}");
        assert_eq!(result["locality"], "MIXED", "{route}: {result}");
        assert_eq!(result["uncertainty"], "NONE", "{route}: {result}");
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn proven_remote_cases_never_classify_local_on_either_route() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    super::endpoints::install(&s);
    prod_context(&s, "tcp://192.0.2.30:2376");
    write(
        &s.home.join("ssh-build01.conf"),
        "Host build01\n HostName 192.0.2.31\n Port 22\n ProxyJump none\n",
    );
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    let ssh = s.home.join("ssh-build01.conf");
    let ssh = ssh.to_str().unwrap();
    let cases: [(&str, &[&str], &str); 6] = [
        ("gh run watch", &["gh", "run", "watch"], "REMOTE_CI"),
        ("gh run view", &["gh", "run", "view"], "REMOTE_CI"),
        ("git push", &["git", "push"], "REMOTE_CI"),
        (
            "ssh build01 make test",
            &["ssh", "-F", ssh, "build01", "make test"],
            "REMOTE_EXEC",
        ),
        (
            "docker prod context",
            &["docker", "--context", "prod", "build", "."],
            "REMOTE_EXEC",
        ),
        (
            "docker selected remote builder",
            &["docker", "buildx", "build", "."],
            "REMOTE_EXEC",
        ),
    ];
    let mut daemon = f.spawn_isolated("test");
    let mut recognized = 0;
    let mut unknown = 0;
    let mut false_local = 0;
    for (name, argv, expected) in cases {
        let before = admission_counts(&f);
        let inspection = s.classify(&f, argv, &[]);
        let (job, _) = s.exec(&f, argv, &[]);
        assert!(!assert_routes(name, &inspection, &job, expected));
        assert_recorded_fixture_child(&s, &job);
        assert_eq!(admission_counts(&f), before, "{name} added local admission");
        match inspection["locality"].as_str().unwrap() {
            "REMOTE" => recognized += 1,
            "UNKNOWN" => unknown += 1,
            "HOST_LOCAL" => false_local += 1,
            other => panic!("unexpected {name} locality: {other}"),
        }
    }
    assert_eq!((s.calls(), recognized, unknown, false_local), (6, 6, 0, 0));
    eprintln!("closing remote cases: denominator 6, recognized-remote 6, UNKNOWN 0, false-local 0 per route");
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn ambiguous_and_local_controls_are_counted_separately() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    super::endpoints::install(&s);
    // A context name, Unix socket or remote API server is not proof of where
    // computation runs. These controls must not inherit a remote decision.
    prod_context(&s, "unix:///tmp/arc-fixture.sock");
    for name in ["act", "wsl.exe", "self-hosted-runner"] {
        let target = if name == "self-hosted-runner" {
            s.repo.join(name)
        } else {
            s.bin.join(name)
        };
        symlink(cli::binary("arc-exec-fixture"), target).unwrap();
    }
    let ssh = s.home.join("ssh.conf");
    let kube = s.home.join("kube.yaml");
    let ssh = ssh.to_str().unwrap();
    let kube = kube.to_str().unwrap();
    let cases: [(&str, &[&str]); 9] = [
        (
            "ssh forwarding",
            &["ssh", "-F", ssh, "-L8080:localhost:80", "worker"],
        ),
        (
            "missing docker context",
            &["docker", "--context", "missing", "build", "."],
        ),
        (
            "mixed buildx nodes",
            &[
                "docker",
                "buildx",
                "build",
                "--builder",
                "mixed-builder",
                ".",
            ],
        ),
        (
            "unproved pod placement",
            &[
                "kubectl",
                "--kubeconfig",
                kube,
                "exec",
                "remote-pod",
                "--",
                "make",
            ],
        ),
        ("unproved local act", &["act", "-j", "build"]),
        ("unproved local wsl", &["wsl.exe", "--", "cargo", "build"]),
        ("unproved local runner", &["./self-hosted-runner"]),
        (
            "prod name with unproved local socket",
            &["docker", "--context", "prod", "build", "."],
        ),
        (
            "conflicting docker selectors",
            &[
                "docker",
                "--context",
                "prod",
                "--host",
                "tcp://192.0.2.30:2376",
                "build",
                ".",
            ],
        ),
    ];
    let mut daemon = f.spawn_isolated("test");
    for (name, argv) in cases {
        let before = admission_counts(&f);
        let inspection = s.classify(&f, argv, &[]);
        let (job, _) = s.exec(&f, argv, &[]);
        assert!(!assert_routes(name, &inspection, &job, "UNKNOWN"));
        assert_recorded_fixture_child(&s, &job);
        assert_eq!(admission_counts(&f), before, "{name} added local admission");
    }
    assert_eq!(s.calls(), 9);
    eprintln!("closing unresolved controls: denominator 9, UNKNOWN 9 per route; separate from remote cases");
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn explicit_prod_context_takes_precedence_over_docker_host_on_both_routes() {
    super::endpoints::explicit_prod_context_takes_precedence_over_docker_host_on_both_routes();
}

#[test]
fn remote_docker_and_ssh_execute_while_local_build_permit_remains_held() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    super::endpoints::install(&s);
    prod_context(&s, "tcp://192.0.2.30:2376");
    write(
        &s.home.join("ssh-build01.conf"),
        "Host build01\n HostName 192.0.2.31\n Port 22\n ProxyJump none\n",
    );
    symlink(cli::binary("arc-exec-fixture"), s.bin.join("cargo")).unwrap();
    let mut daemon = f.spawn_isolated("test");
    let listener = std::os::unix::net::UnixListener::bind(f.root.join("build-control")).unwrap();
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
        db.query_row(
            "SELECT (SELECT count(*) FROM traffic_entries), \
             (SELECT count(*) FROM traffic_entries WHERE state='QUEUED'), \
             (SELECT count(*) FROM permits), \
             (SELECT count(*) FROM permits WHERE state='ACTIVE'), \
             (SELECT count(*) FROM jobs WHERE queue_at IS NOT NULL OR reserved_at IS NOT NULL)",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .unwrap()
    };
    let before = counts();
    assert_eq!((before.1, before.2, before.3), (0, 1, 1));
    let ssh = s.home.join("ssh-build01.conf");
    let ssh = ssh.to_str().unwrap();
    for (name, argv) in [
        (
            "remote prod context",
            &["docker", "--context", "prod", "build", "."][..],
        ),
        (
            "remote selected buildx",
            &["docker", "buildx", "build", "."][..],
        ),
        (
            "remote ssh",
            &["ssh", "-F", ssh, "build01", "make test"][..],
        ),
    ] {
        let inspection = s.classify(&f, argv, &[]);
        let (job, _) = s.exec(&f, argv, &[]);
        assert!(!assert_routes(name, &inspection, &job, "REMOTE_EXEC"));
        assert_recorded_fixture_child(&s, &job);
        assert_eq!(counts(), before, "{name} changed local admission state");
    }
    assert_eq!(s.calls(), 3);
    drop(db);
    release.write_all(b"x").unwrap();
    let output = local.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(17));
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn proven_local_cargo_control_uses_local_build_admission_on_both_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    symlink(cli::binary("arc-exec-fixture"), s.bin.join("cargo")).unwrap();
    let mut daemon = f.spawn_isolated("test");
    let before = admission_counts(&f);
    let argv = ["cargo", "build"];
    let inspection = s.classify(&f, &argv, &[]);
    let (job, _) = s.exec(&f, &argv, &[]);
    for result in [&inspection, &job["classification"]] {
        assert_eq!(result["classification"], "BUILD");
        assert_eq!(result["locality"], "HOST_LOCAL");
        assert_eq!(result["uncertainty"], "NONE");
    }
    assert_recorded_fixture_child(&s, &job);
    assert_eq!(s.calls(), 1);
    let after = admission_counts(&f);
    assert_eq!((after.0 - before.0, after.1 - before.1), (1, 1));
    eprintln!(
        "closing local controls: denominator 1, HOST_LOCAL 1 per route; separate from remote cases"
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn forwarded_docker_socket_is_not_contacted_or_treated_as_local() {
    docker_controls::forwarded_docker_socket_is_not_contacted_or_treated_as_local();
}

#[cfg(target_os = "macos")]
#[test]
fn docker_local_listener_controls_use_both_production_routes_and_admission() {
    docker_controls::docker_local_listener_controls_use_both_production_routes_and_admission();
}
