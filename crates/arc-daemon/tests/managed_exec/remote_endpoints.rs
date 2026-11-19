//! All addresses and endpoint facts here are simulated; no remote service is contacted.
use super::{protocol, runtime::Fixture, support};
use arc_ipc::{locality::OPERATION_CLASSIFY, Request};
use serde_json::{json, Value};

struct Case {
    id: &'static str,
    shell: &'static str,
    argv: &'static [&'static str],
    context: Value,
}

fn fact(kind: &str, selector: &str, endpoints: &[&str]) -> Value {
    json!({"kind":kind,"selector":selector,"endpoints":endpoints})
}

fn context(mut facts: Vec<Value>) -> Value {
    // Simulated complete local address inventory, not an assertion about a real host.
    facts.push(fact(
        "SSH_DESTINATION",
        "@local-addresses",
        &["192.0.2.5", "::1"],
    ));
    json!({"captured_at_ms":10000,"environment":{},"facts":facts})
}

fn ssh_context() -> Value {
    context(vec![
        fact("SSH_DESTINATION", "build01", &["ssh://192.0.2.30:22"]),
        fact("SSH_DESTINATION", "user@build01", &["ssh://192.0.2.30:22"]),
        fact("SSH_DESTINATION", "192.0.2.30", &["ssh://192.0.2.30:22"]),
    ])
}

fn docker_context(selector: &str, endpoint: &str) -> Value {
    context(vec![
        fact("DOCKER_CONTEXT", selector, &[endpoint]),
        fact("DOCKER_BUILDER", selector, &[endpoint]),
    ])
}

fn builder_context(selector: &str, endpoints: &[&str]) -> Value {
    context(vec![fact("DOCKER_BUILDER", selector, endpoints)])
}

fn kube_context() -> Value {
    context(vec![
        fact("KUBERNETES_CONTEXT", "prod", &["https://192.0.2.60:6443"]),
        fact("KUBERNETES_POD", "prod/default/remote-pod", &["192.0.2.50"]),
    ])
}

fn endpoint_cases() -> Vec<Case> {
    let s = ssh_context();
    let d = docker_context("prod", "tcp://192.0.2.40:2376");
    let b = builder_context("remote", &["tcp://192.0.2.40:2376"]);
    let k = kube_context();
    let mut denv = d.clone();
    denv["environment"] = json!({"DOCKER_CONTEXT":"prod"});
    let mut benv = b.clone();
    benv["environment"] = json!({"BUILDX_BUILDER":"remote"});
    macro_rules! case {
        ($id:literal, $shell:literal, [$($arg:literal),+], $context:expr) => {
            Case { id:$id, shell:$shell, argv:&[$($arg),+], context:$context.clone() }
        };
    }
    // Keep the named denominator readable as a table; each argv is explicit.
    #[rustfmt::skip]
    let cases = vec![
        case!("EP01", "ssh build01 'make test'", ["ssh","build01","make test"], s),
        case!("EP02", "ssh user@build01 make test", ["ssh","user@build01","make","test"], s),
        case!("EP03", "ssh -- build01 cargo build", ["ssh","--","build01","cargo","build"], s),
        case!("EP04", "ssh -p 22 build01 make", ["ssh","-p","22","build01","make"], s),
        case!("EP05", "ssh 192.0.2.30 make", ["ssh","192.0.2.30","make"], s),
        case!("EP06", "ssh build01 -- make test", ["ssh","build01","--","make","test"], s),
        case!("EP07", "ssh build01 'cargo test --locked'", ["ssh","build01","cargo test --locked"], s),
        case!("EP08", "docker --context prod build .", ["docker","--context","prod","build","."], d),
        case!("EP09", "docker --context prod build -t demo .", ["docker","--context","prod","build","-t","demo","."], d),
        case!("EP10", "docker build .", ["docker","build","."], denv),
        case!("EP11", "docker build -f Containerfile .", ["docker","build","-f","Containerfile","."], denv),
        case!("EP12", "docker --host tcp://192.0.2.40:2376 build .", ["docker","--host","tcp://192.0.2.40:2376","build","."], docker_context("tcp://192.0.2.40:2376", "tcp://192.0.2.40:2376")),
        case!("EP13", "docker --context prod build --no-cache .", ["docker","--context","prod","build","--no-cache","."], d),
        case!("EP14", "docker buildx build .", ["docker","buildx","build","."], benv),
        case!("EP15", "docker buildx build --builder remote .", ["docker","buildx","build","--builder","remote","."], b),
        case!("EP16", "docker buildx build --builder remote -t demo .", ["docker","buildx","build","--builder","remote","-t","demo","."], b),
        case!("EP17", "docker buildx build --builder remote --platform linux/arm64 .", ["docker","buildx","build","--builder","remote","--platform","linux/arm64","."], b),
        case!("EP18", "docker buildx build --builder remote --push .", ["docker","buildx","build","--builder","remote","--push","."], b),
        case!("EP19", "docker buildx build --builder remote --no-cache .", ["docker","buildx","build","--builder","remote","--no-cache","."], b),
        case!("EP20", "kubectl --context prod exec remote-pod -- make", ["kubectl","--context","prod","exec","remote-pod","--","make"], k),
        case!("EP21", "kubectl --context prod exec remote-pod -- make test", ["kubectl","--context","prod","exec","remote-pod","--","make","test"], k),
        case!("EP22", "kubectl --context prod -n default exec remote-pod -- cargo build", ["kubectl","--context","prod","-n","default","exec","remote-pod","--","cargo","build"], k),
    ];
    cases
}

fn classify(command: Value, context: &Value, now: u64) -> Value {
    let mut request = Request::new("simulated-endpoint", OPERATION_CLASSIFY);
    request.payload = json!({"command":command,"context":context});
    let response = arc_ipc::locality::classify_response_at(&request, now);
    assert_eq!(response.status, "ok", "{response:?}");
    response.result.unwrap()
}

fn both(shell: &str, argv: &[&str], context: &Value, expected: &str) {
    for command in [
        json!({"kind":"SHELL","shell_kind":"sh","command":shell}),
        json!({"kind":"ARGV","argv":argv}),
    ] {
        let result = classify(command, context, 10_000);
        assert_eq!(result["classification"], expected, "{shell}: {result}");
        let (locality, uncertainty) = match expected {
            "REMOTE_EXEC" => ("REMOTE", "NONE"),
            "BUILD" | "TEST" => ("HOST_LOCAL", "NONE"),
            _ => ("UNKNOWN", "OBSERVE"),
        };
        assert_eq!(result["locality"], locality, "{shell}: {result}");
        assert_eq!(result["uncertainty"], uncertainty, "{shell}: {result}");
    }
}

#[test]
fn named_simulated_srs_endpoint_denominator_has_zero_false_local() {
    let cases = endpoint_cases();
    assert_eq!(cases.len(), 22);
    let mut false_local = 0;
    for case in &cases {
        for command in [
            json!({"kind":"SHELL","shell_kind":"sh","command":case.shell}),
            json!({"kind":"ARGV","argv":case.argv}),
        ] {
            let result = classify(command, &case.context, 10_000);
            if result["locality"] == "HOST_LOCAL" {
                false_local += 1;
            }
            assert_eq!(
                result["classification"], "REMOTE_EXEC",
                "{}: {result}",
                case.id
            );
            assert_eq!(result["locality"], "REMOTE", "{}: {result}", case.id);
            assert_eq!(result["uncertainty"], "NONE", "{}: {result}", case.id);
            assert_eq!(
                result["reason"],
                if case.id <= "EP07" {
                    "SSH_REMOTE_COMMAND"
                } else {
                    "REMOTE_ENDPOINT_EXEC"
                }
            );
        }
    }
    assert_eq!(false_local, 0, "false-local endpoint fixtures / 22");
    eprintln!("simulated SRS endpoints EP01–EP22: false-local {false_local}/22, shell and argv");
}

#[test]
fn precedence_local_and_ambiguous_controls() {
    let plain_docker = ["docker", "build", "."];
    let mut settings = docker_context("prod", "tcp://192.0.2.40:2376");
    settings["facts"].as_array_mut().unwrap().extend([
        fact("DOCKER_CONTEXT", "local", &["unix:///var/run/docker.sock"]),
        fact("DOCKER_BUILDER", "local", &["unix:///var/run/docker.sock"]),
    ]);
    settings["environment"] =
        json!({"DOCKER_CONTEXT":"prod","DOCKER_HOST":"tcp://192.0.2.40:2376"});
    both(
        "docker --context local build .",
        &["docker", "--context", "local", "build", "."],
        &settings,
        "BUILD",
    );
    settings["environment"]["DOCKER_CONTEXT"] = json!("local");
    both(
        "docker --context prod build .",
        &["docker", "--context", "prod", "build", "."],
        &settings,
        "REMOTE_EXEC",
    );
    both("docker build .", &plain_docker, &settings, "BUILD");
    both(
        "docker --context prod --host tcp://192.0.2.40:2376 build .",
        &[
            "docker",
            "--context",
            "prod",
            "--host",
            "tcp://192.0.2.40:2376",
            "build",
            ".",
        ],
        &settings,
        "UNKNOWN",
    );
    both(
        "docker --context prod build --unknown .",
        &["docker", "--context", "prod", "build", "--unknown", "."],
        &settings,
        "UNKNOWN",
    );
    both(
        "/tmp/docker --context local build .",
        &["/tmp/docker", "--context", "local", "build", "."],
        &settings,
        "UNKNOWN",
    );
    both("cargo build", &["cargo", "build"], &settings, "BUILD");
    let mut legacy_engine = settings.clone();
    legacy_engine["environment"]["DOCKER_BUILDKIT"] = json!("0");
    both(
        "docker --context local build .",
        &["docker", "--context", "local", "build", "."],
        &legacy_engine,
        "UNKNOWN",
    );

    let mut buildx = docker_context("@current", "unix:///var/run/docker.sock");
    buildx["facts"].as_array_mut().unwrap().extend([
        fact("DOCKER_CONTEXT", "prod", &["tcp://192.0.2.40:2376"]),
        fact("DOCKER_BUILDER", "prod", &["unix:///var/run/docker.sock"]),
        fact(
            "DOCKER_BUILDER",
            "@selected/prod",
            &["unix:///var/run/docker.sock"],
        ),
        fact("DOCKER_BUILDER", "remote", &["tcp://192.0.2.40:2376"]),
        fact("DOCKER_BUILDER", "local", &["unix:///var/run/docker.sock"]),
        fact(
            "DOCKER_BUILDER",
            "mixed",
            &["unix:///var/run/docker.sock", "tcp://192.0.2.40:2376"],
        ),
    ]);
    // Each row is a simulated effective placement, exercised through both routes.
    #[rustfmt::skip]
    let cases: Vec<(Value, &str, &[&str], &str)> = vec![
        (json!({"DOCKER_CONTEXT":"prod", "BUILDX_BUILDER":"local"}), "docker build .", &["docker", "build", "."], "UNKNOWN"),
        (json!({"DOCKER_CONTEXT":"prod", "BUILDX_BUILDER":"local", "DOCKER_BUILDKIT":""}), "docker build .", &["docker", "build", "."], "UNKNOWN"),
        (json!({"DOCKER_CONTEXT":"prod", "BUILDX_BUILDER":"local", "DOCKER_BUILDKIT":"1"}), "docker build .", &["docker", "build", "."], "UNKNOWN"),
        (json!({"BUILDX_BUILDER":"remote"}), "docker build .", &["docker", "build", "."], "UNKNOWN"),
        (json!({"BUILDX_BUILDER":"remote", "DOCKER_BUILDKIT":"1"}), "docker build .", &["docker", "build", "."], "UNKNOWN"),
        (json!({"DOCKER_CONTEXT":"prod", "BUILDX_BUILDER":"remote"}), "docker build .", &["docker", "build", "."], "REMOTE_EXEC"),
        (json!({"BUILDX_BUILDER":"local"}), "docker build .", &["docker", "build", "."], "BUILD"),
        (json!({"DOCKER_CONTEXT":"prod", "BUILDX_BUILDER":"local"}), "docker build --builder remote .", &["docker", "build", "--builder", "remote", "."], "REMOTE_EXEC"),
        (json!({"BUILDX_BUILDER":"remote"}), "docker build --builder local .", &["docker", "build", "--builder", "local", "."], "BUILD"),
        (json!({"BUILDX_BUILDER":"remote"}), "docker buildx build --builder local .", &["docker", "buildx", "build", "--builder", "local", "."], "BUILD"),
        (json!({"BUILDX_BUILDER":"local"}), "docker buildx build --builder remote .", &["docker", "buildx", "build", "--builder", "remote", "."], "REMOTE_EXEC"),
        (json!({"DOCKER_CONTEXT":"prod", "BUILDX_BUILDER":"remote"}), "docker buildx build --builder local .", &["docker", "buildx", "build", "--builder", "local", "."], "BUILD"),
        (json!({"BUILDX_BUILDER":"local", "DOCKER_BUILDKIT":"0"}), "docker buildx build --builder remote .", &["docker", "buildx", "build", "--builder", "remote", "."], "REMOTE_EXEC"),
        (json!({"DOCKER_CONTEXT":"prod", "BUILDX_BUILDER":"remote", "DOCKER_BUILDKIT":"0"}), "docker buildx build --builder local .", &["docker", "buildx", "build", "--builder", "local", "."], "BUILD"),
        (json!({"DOCKER_BUILDKIT":"0", "BUILDX_BUILDER":"remote"}), "docker buildx build .", &["docker", "buildx", "build", "."], "REMOTE_EXEC"),
        (json!({"DOCKER_CONTEXT":"prod", "DOCKER_BUILDKIT":"0", "BUILDX_BUILDER":"local"}), "docker buildx build .", &["docker", "buildx", "build", "."], "BUILD"),
        (json!({"DOCKER_CONTEXT":"prod", "DOCKER_BUILDKIT":"0"}), "docker buildx build .", &["docker", "buildx", "build", "."], "BUILD"),
        (json!({"DOCKER_CONTEXT":"prod"}), "docker build --builder local .", &["docker", "build", "--builder", "local", "."], "UNKNOWN"),
        (json!({}), "docker --context prod buildx build --builder local .", &["docker", "--context", "prod", "buildx", "build", "--builder", "local", "."], "BUILD"),
        (json!({}), "docker build --builder remote .", &["docker", "build", "--builder", "remote", "."], "UNKNOWN"),
        (json!({"DOCKER_CONTEXT":"prod"}), "docker build .", &["docker", "build", "."], "UNKNOWN"),
        (json!({"BUILDX_BUILDER":"remote"}), "docker buildx build --builder mixed .", &["docker", "buildx", "build", "--builder", "mixed", "."], "UNKNOWN"),
        (json!({}), "docker buildx build --builder missing .", &["docker", "buildx", "build", "--builder", "missing", "."], "UNKNOWN"),
        (json!({"BUILDX_BUILDER":"local"}), "docker buildx build .", &["docker", "buildx", "build", "."], "BUILD"),
        (json!({}), "docker buildx build .", &["docker", "buildx", "build", "."], "UNKNOWN"),
    ];
    for (environment, shell, argv, expected) in cases {
        buildx["environment"] = environment;
        both(shell, argv, &buildx, expected);
    }
    let mut independent = builder_context("local", &["unix:///var/run/docker.sock"]);
    independent["environment"] = json!({"BUILDX_BUILDER":"local"});
    both("docker build .", &plain_docker, &independent, "UNKNOWN");
    independent["environment"] = json!({"BUILDX_BUILDER":"local", "DOCKER_BUILDKIT":"0"});
    let buildx_shell = "docker buildx build .";
    let buildx_argv = ["docker", "buildx", "build", "."];
    both(buildx_shell, &buildx_argv, &independent, "BUILD");
    let unresolved_engine = fact("DOCKER_CONTEXT", "@current", &["unresolvable"]);
    independent["facts"]
        .as_array_mut()
        .unwrap()
        .push(unresolved_engine);
    both(buildx_shell, &buildx_argv, &independent, "UNKNOWN");
    let s = ssh_context();
    let mut no_local_inventory = s.clone();
    no_local_inventory["facts"].as_array_mut().unwrap().pop();
    let mut same_host = s.clone();
    same_host["facts"][3]["endpoints"] = json!(["192.0.2.30"]);
    for (shell, argv) in [
        ("ssh build01", vec!["ssh", "build01"]),
        (
            "ssh -p 23 build01 make",
            vec!["ssh", "-p", "23", "build01", "make"],
        ),
        (
            "ssh -L 1234:localhost:80 build01 make",
            vec!["ssh", "-L", "1234:localhost:80", "build01", "make"],
        ),
        (
            "ssh -o ProxyCommand=other build01 make",
            vec!["ssh", "-o", "ProxyCommand=other", "build01", "make"],
        ),
        ("ssh missing make", vec!["ssh", "missing", "make"]),
    ] {
        both(shell, &argv, &s, "UNKNOWN");
    }
    let loopback = context(vec![fact(
        "SSH_DESTINATION",
        "build01",
        &["ssh://127.0.0.1:22"],
    )]);
    let ssh_argv = ["ssh", "build01", "make"];
    for control in [&no_local_inventory, &same_host, &loopback] {
        both("ssh build01 make", &ssh_argv, control, "UNKNOWN");
    }

    let kube_shell = "kubectl --context prod exec remote-pod -- make";
    #[rustfmt::skip]
    let kube_argv = ["kubectl", "--context", "prod", "exec", "remote-pod", "--", "make"];
    let mut missing = kube_context();
    missing["facts"].as_array_mut().unwrap().remove(1);
    both(kube_shell, &kube_argv, &missing, "UNKNOWN");
    let mut invalid_server = kube_context();
    invalid_server["facts"][0]["endpoints"] = json!(["unresolvable"]);
    both(kube_shell, &kube_argv, &invalid_server, "UNKNOWN");
    let mut conflicting = kube_context();
    conflicting["facts"].as_array_mut().unwrap().push(fact(
        "KUBERNETES_POD",
        "prod/other/remote-pod",
        &["192.0.2.51"],
    ));
    both(kube_shell, &kube_argv, &conflicting, "UNKNOWN");
    both(
        "kubectl --context prod -n default exec remote-pod -- make",
        &[
            "kubectl",
            "--context",
            "prod",
            "-n",
            "default",
            "exec",
            "remote-pod",
            "--",
            "make",
        ],
        &conflicting,
        "REMOTE_EXEC",
    );
    both(
        "kubectl --server https://other exec remote-pod -- make",
        &[
            "kubectl",
            "--server",
            "https://other",
            "exec",
            "remote-pod",
            "--",
            "make",
        ],
        &kube_context(),
        "UNKNOWN",
    );
    both(
        "custom-program build",
        &["custom-program", "build"],
        &settings,
        "UNKNOWN",
    );
}

#[test]
fn stale_future_and_mixed_shell_are_deterministic() {
    let c = ssh_context();
    let command = json!({"kind":"SHELL","shell_kind":"sh","command":"ssh build01 make"});
    for now in [9_999, 15_001] {
        let result = classify(command.clone(), &c, now);
        assert_eq!(result["classification"], "UNKNOWN");
        assert_eq!(result["uncertainty"], "OBSERVE");
    }
    assert_eq!(
        classify(command, &c, 15_000)["classification"],
        "REMOTE_EXEC"
    );
    let mixed = classify(
        json!({"kind":"SHELL","shell_kind":"sh","command":"cargo test && ssh build01 make"}),
        &c,
        10_000,
    );
    assert_eq!(mixed["classification"], "MIXED");
    assert_eq!(mixed["locality"], "MIXED");
    let unresolved = classify(
        json!({"kind":"SHELL","shell_kind":"sh","command":"cargo test && ssh missing make"}),
        &c,
        10_000,
    );
    assert_eq!(unresolved["classification"], "UNKNOWN");
}

#[test]
fn real_daemon_classifies_simulated_endpoints_without_creating_reservations() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    for case in endpoint_cases() {
        for command in [
            json!({"kind":"SHELL","shell_kind":"sh","command":case.shell}),
            json!({"kind":"ARGV","argv":case.argv}),
        ] {
            let mut current = case.context.clone();
            current["captured_at_ms"] = json!(arc_ipc::locality::now_ms().unwrap());
            let response = protocol::call(
                &f,
                OPERATION_CLASSIFY,
                json!({"command":command,"context":current}),
            );
            assert_eq!(response.status, "ok", "{}: {response:?}", case.id);
            let result = response.result.unwrap();
            assert_eq!(
                result["classification"], "REMOTE_EXEC",
                "{}: {result}",
                case.id
            );
            assert_eq!(result["locality"], "REMOTE", "{}: {result}", case.id);
        }
    }
    let db = rusqlite::Connection::open(&f.database).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM jobs", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM permits", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn unresolved_endpoint_stubs_execute_once_without_a_permit() {
    use std::{fs, os::unix::fs::symlink, process::Stdio};
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    for (name, args) in [
        ("ssh", vec!["missing", "make"]),
        ("docker", vec!["build", "."]),
        ("kubectl", vec!["exec", "missing", "--", "make"]),
    ] {
        symlink(super::cli::binary("arc-exec-fixture"), f.root.join(name)).unwrap();
        let marker = f.root.join(format!("{name}-calls"));
        let mut command = super::cli::command(&f);
        command
            .args(["exec", "--", name])
            .args(&args)
            .current_dir(&f.root)
            .env("PATH", &f.root)
            .env("ARC_FIXTURE_MODE", "echo")
            .env("ARC_FIXTURE_MARKER", &marker)
            .env_remove("DOCKER_CONTEXT")
            .env_remove("DOCKER_HOST")
            .env_remove("BUILDX_BUILDER")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = super::runtime::OwnedChild::new(command.spawn().unwrap());
        super::cli::wait(&mut child);
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(17), "{name}: {output:?}");
        assert_eq!(output.stderr, b"fixture-stderr\n");
        assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
        let job = protocol::status(&f, &super::cli::recent(&f));
        assert_eq!(
            job.classification.classification,
            arc_ipc::JobClass::Unknown
        );
        assert_eq!(
            job.classification.uncertainty,
            arc_ipc::Uncertainty::Observe
        );
    }
    let db = rusqlite::Connection::open(&f.database).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM jobs WHERE queue_at IS NOT NULL OR reserved_at IS NOT NULL",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM permits", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}
