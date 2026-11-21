//! Built arcctl, real daemon and offline configuration fixtures.
use super::{cli, protocol, runtime::Fixture, support};
use arc_ipc::jobs::{JobId, StatusRequest, OPERATION_JOB_STATUS_LOCALITY};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{ffi::OsStrExt, fs::symlink},
    path::Path,
    process::{Command, Stdio},
};

#[path = "../support/child_watchdog.rs"]
#[allow(clippy::duplicate_mod)] // The independently runnable fixture shares the watchdog.
mod child_watchdog;

#[test]
fn unix_socket_facts_do_not_establish_compute_locality() {
    let mut request = arc_ipc::Request::new("local", arc_ipc::locality::OPERATION_CLASSIFY);
    request.payload = json!({
        "command":{"kind":"ARGV", "argv":["docker", "--context", "local", "build", "."]},
        "context":{"captured_at_ms":10000,"facts":[
            {"kind":"DOCKER_CONTEXT","selector":"local","endpoints":["unix:///tmp/fixture-docker.sock"]},
            {"kind":"DOCKER_BUILDER","selector":"local","endpoints":["unix:///tmp/fixture-docker.sock"]}
        ]}
    });
    let result = arc_ipc::locality::classify_response_at(&request, 10000)
        .result
        .unwrap();
    assert_eq!(
        result,
        json!({
            "classification":"UNKNOWN", "locality":"UNKNOWN",
            "reason":"ENDPOINT_AMBIGUOUS", "uncertainty":"OBSERVE"
        })
    );
}

fn write(path: &Path, content: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn context(root: &Path, name: &str, hash: &str, endpoint: &str) {
    write(
        &root
            .join(".docker/contexts/meta")
            .join(hash)
            .join("meta.json"),
        json!({"Name":name,"Endpoints":{"docker":{"Host":endpoint}}}).to_string(),
    );
}

fn fixture(f: &Fixture) {
    super::capture_integration::fixture_interfaces(f);
    for name in ["docker", "ssh", "kubectl"] {
        symlink(cli::binary("arc-exec-fixture"), f.root.join(name)).unwrap();
    }
    write(
        &f.root.join(".docker/config.json"),
        r#"{"currentContext":"remote"}"#,
    );
    context(
        &f.root,
        "remote",
        "b71199ebd070b36beab7317920c2c2f1d777df8d05e5527d8458fda57cb17a7a",
        "tcp://192.0.2.20:2376",
    );
    context(
        &f.root,
        "local",
        "25bf8e1a2393f1108d37029b3df5593236c755742ec93465bbafa9b290bddcf6",
        "unix:///tmp/arc-fixture.sock",
    );
    write(&f.root.join(".docker/buildx/instances/remote-builder"),
        json!({"Name":"remote-builder","Driver":"remote","Dynamic":false,"Nodes":[{"Name":"node0","Endpoint":"tcp://192.0.2.21:1234"}]}).to_string());
    write(&f.root.join(".docker/buildx/instances/local-builder"),
        json!({"Name":"local-builder","Driver":"docker-container","Dynamic":false,"Nodes":[{"Name":"node0","Endpoint":"local"}]}).to_string());
    write(&f.root.join(".docker/buildx/instances/mixed-builder"),
        json!({"Name":"mixed-builder","Driver":"remote","Dynamic":false,"Nodes":[{"Name":"node0","Endpoint":"tcp://192.0.2.21:1234"},{"Name":"node1","Endpoint":"unix:///tmp/arc-fixture.sock"}]}).to_string());
    write(&f.root.join("custom-buildx/instances/custom-builder"),
        json!({"Name":"custom-builder","Driver":"remote","Dynamic":false,"Nodes":[{"Name":"node0","Endpoint":"tcp://192.0.2.22:1234"}]}).to_string());
    write(
        &f.root.join(".docker/buildx/defaults/b71199ebd070b36beab7"),
        "remote-builder",
    );
    write(&f.root.join("ssh.conf"), "Host worker\n HostName 192.0.2.24\n Port 2222\n ProxyJump none\nHost *\n HostName 192.0.2.25\n Port 22\n");
    write(
        &f.root.join("ssh-proxy.conf"),
        "Host worker\n HostName 192.0.2.24\n ProxyJump jump\n",
    );
    write(
        &f.root.join("ssh-local.conf"),
        "Host worker\n HostName 127.0.0.1\n",
    );
    write(&f.root.join("kube.yaml"), "current-context: first\ncontexts:\n- name: first\n  context:\n    cluster: alpha\n- name: second\n  context:\n    cluster: beta\nclusters:\n- name: alpha\n  cluster:\n    server: https://192.0.2.41:6443\n- name: beta\n  cluster:\n    server: https://192.0.2.42:6443\n");
}

fn run(
    f: &Fixture,
    name: &str,
    args: &[&str],
    env: &[(&str, &str)],
    marker: &Path,
) -> (Value, JobId) {
    let mut cmd = Command::new(cli::binary("arcctl"));
    cmd.env_clear()
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env("HOME", &f.root)
        .env("PATH", &f.root)
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", marker)
        .env("PRIVATE_ENV", "offline-fixture-value")
        .current_dir(&f.root)
        .args(["exec", "--", name])
        .args(args)
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    super::capture_integration::use_fixture_interfaces(&mut cmd, f);
    let mut child = child_watchdog::Owned::spawn(&mut cmd).unwrap();
    child
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"fixture stdin\n")
        .unwrap();
    let status = child.wait("offline endpoint CLI").unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();
    child
        .child
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut stderr)
        .unwrap();
    assert_eq!(status.code(), Some(17), "{name}: {status:?}");
    assert_eq!(stderr, b"fixture-stderr\n");
    let metadata: Value =
        serde_json::from_slice(stdout.strip_prefix(b"fixture stdin\n").unwrap()).unwrap();
    let expected_args: Vec<Vec<u8>> = args.iter().map(|arg| arg.as_bytes().to_vec()).collect();
    assert_eq!(metadata["args"], json!(expected_args));
    assert_eq!(
        metadata["cwd"],
        json!(fs::canonicalize(&f.root).unwrap().as_os_str().as_bytes())
    );
    assert_eq!(metadata["environment"], json!(b"offline-fixture-value"));
    #[path = "capture_integration/closing_locality/marker.rs"]
    mod marker;
    marker::check(f, name, args, marker);
    let id = cli::recent(f);
    let response = protocol::call(
        f,
        OPERATION_JOB_STATUS_LOCALITY,
        json!(StatusRequest::ById { job_id: id.clone() }),
    );
    assert_eq!(response.status, "ok", "{response:?}");
    (response.result.unwrap()["job"].clone(), id)
}

fn classification(job: &Value, expected: &str) {
    assert_eq!(job["classification"]["classification"], expected, "{job}");
    assert_eq!(
        job["classification"]["uncertainty"],
        if expected == "UNKNOWN" {
            "OBSERVE"
        } else {
            "NONE"
        }
    );
}

fn classify_shell(f: &Fixture, name: &str, args: &[&str], env: &[(&str, &str)]) -> Value {
    let words = std::iter::once(name).chain(args.iter().copied());
    let shell = words
        .map(|word| {
            assert!(!word.contains('\''), "fixture words must be safely quoted");
            format!("'{word}'")
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut cmd = Command::new(cli::binary("arcctl"));
    cmd.env_clear()
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env("HOME", &f.root)
        .env("PATH", &f.root)
        .current_dir(&f.root)
        .envs(env.iter().copied())
        .args(["classify", "--json", "--", &shell]);
    super::capture_integration::use_fixture_interfaces(&mut cmd, f);
    let output = child_watchdog::output(&mut cmd, "offline endpoint classify").unwrap();
    assert!(output.status.success(), "{name}: {output:?}");
    assert!(output.stderr.is_empty(), "{name}: {output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

type Case<'a> = (&'a str, Vec<&'a str>, Vec<(&'a str, &'a str)>, &'a str);

#[test]
fn offline_ssh_docker_kube_precedence_and_uncertainty() {
    let _serial = support::serial();
    let f = Fixture::new();
    fixture(&f);
    let mut daemon = f.spawn("test");
    let ssh_config = f.root.join("ssh.conf");
    let proxy_config = f.root.join("ssh-proxy.conf");
    let local_config = f.root.join("ssh-local.conf");
    let kube = f.root.join("kube.yaml");
    let missing = f.root.join("missing.yaml");
    let docker_config = f.root.join(".docker");
    let buildx_config = f.root.join("custom-buildx");
    let cases: Vec<Case<'_>> = vec![
        (
            "ssh",
            vec!["-F", ssh_config.to_str().unwrap(), "worker", "make"],
            vec![],
            "REMOTE_EXEC",
        ),
        (
            "ssh",
            vec![
                "-F",
                ssh_config.to_str().unwrap(),
                "-p",
                "2299",
                "worker",
                "make",
            ],
            vec![],
            "REMOTE_EXEC",
        ),
        (
            "ssh",
            vec!["-F", proxy_config.to_str().unwrap(), "worker", "make"],
            vec![],
            "UNKNOWN",
        ),
        (
            "ssh",
            vec!["-F", missing.to_str().unwrap(), "worker", "make"],
            vec![],
            "UNKNOWN",
        ),
        (
            "ssh",
            vec!["-F", local_config.to_str().unwrap(), "worker", "make"],
            vec![],
            "UNKNOWN",
        ),
        ("docker", vec!["build", "."], vec![], "REMOTE_EXEC"),
        (
            "docker",
            vec!["--context", "local", "build", "."],
            vec![],
            "UNKNOWN",
        ),
        (
            "docker",
            vec!["build", "."],
            vec![("DOCKER_HOST", "unix:///tmp/forwarded-fixture.sock")],
            "UNKNOWN",
        ),
        (
            "docker",
            vec!["buildx", "build", "--builder", "local-builder", "."],
            vec![("DOCKER_HOST", "unix:///tmp/forwarded-fixture.sock")],
            "UNKNOWN",
        ),
        (
            "docker",
            vec!["build", "."],
            vec![
                ("DOCKER_CONTEXT", "local"),
                ("DOCKER_HOST", "tcp://192.0.2.20:2376"),
            ],
            "REMOTE_EXEC",
        ),
        (
            "docker",
            vec!["--context", "remote", "build", "."],
            vec![("DOCKER_HOST", "unix:///tmp/arc-fixture.sock")],
            "REMOTE_EXEC",
        ),
        (
            "docker",
            vec!["--config", docker_config.to_str().unwrap(), "build", "."],
            vec![("DOCKER_CONFIG", missing.to_str().unwrap())],
            "REMOTE_EXEC",
        ),
        (
            "docker",
            vec!["build", "."],
            vec![("DOCKER_HOST", "ssh://192.0.2.20:22")],
            "UNKNOWN",
        ),
        (
            "docker",
            vec!["--context", "missing", "build", "."],
            vec![],
            "UNKNOWN",
        ),
        (
            "docker",
            vec![
                "--context",
                "remote",
                "--host",
                "tcp://192.0.2.20:2376",
                "build",
                ".",
            ],
            vec![],
            "UNKNOWN",
        ),
        (
            "docker",
            vec!["buildx", "build", "--builder", "remote-builder", "."],
            vec![("BUILDX_BUILDER", "local-builder")],
            "REMOTE_EXEC",
        ),
        (
            "docker",
            vec![
                "--context",
                "local",
                "buildx",
                "--builder",
                "remote-builder",
                "build",
                ".",
            ],
            vec![],
            "REMOTE_EXEC",
        ),
        (
            "docker",
            vec!["buildx", "build", "--builder", "mixed-builder", "."],
            vec![],
            "UNKNOWN",
        ),
        (
            "docker",
            vec!["buildx", "build", "."],
            vec![],
            "REMOTE_EXEC",
        ),
        (
            "docker",
            vec!["buildx", "build", "."],
            vec![
                ("BUILDX_CONFIG", buildx_config.to_str().unwrap()),
                ("BUILDX_BUILDER", "custom-builder"),
            ],
            "REMOTE_EXEC",
        ),
        (
            "docker",
            vec!["buildx", "build", "--builder", "custom-builder", "."],
            vec![],
            "UNKNOWN",
        ),
        (
            "docker",
            vec!["buildx", "build", "--builder", "missing", "."],
            vec![],
            "UNKNOWN",
        ),
        (
            "kubectl",
            vec![
                "--kubeconfig",
                kube.to_str().unwrap(),
                "--context",
                "second",
                "exec",
                "pod",
                "--",
                "make",
            ],
            vec![],
            "UNKNOWN",
        ),
        (
            "kubectl",
            vec![
                "--kubeconfig",
                missing.to_str().unwrap(),
                "exec",
                "pod",
                "--",
                "make",
            ],
            vec![],
            "UNKNOWN",
        ),
        (
            "kubectl",
            vec![
                "--kubeconfig",
                kube.to_str().unwrap(),
                "--cluster",
                "beta",
                "exec",
                "pod",
                "--",
                "make",
            ],
            vec![],
            "UNKNOWN",
        ),
        (
            "kubectl",
            vec![
                "--kubeconfig",
                kube.to_str().unwrap(),
                "--server=https://192.0.2.43:6443",
                "exec",
                "pod",
                "--",
                "make",
            ],
            vec![],
            "UNKNOWN",
        ),
        (
            "kubectl",
            vec![
                "--kubeconfig",
                kube.to_str().unwrap(),
                "--context",
                "first",
                "--context",
                "second",
                "exec",
                "pod",
                "--",
                "make",
            ],
            vec![],
            "UNKNOWN",
        ),
    ];
    let (mut remote, mut unknown, mut local, mut false_local, mut shell_unknown) = (0, 0, 0, 0, 0);
    for (index, (name, args, env, expected)) in cases.iter().enumerate() {
        let marker = f.root.join(format!("call-{index}"));
        let (job, _) = run(&f, name, args, env, &marker);
        classification(&job, expected);
        // The shared predecessor's protocol-1 classify route already
        // recognizes these controls as unknown; proven remote classify is
        // exercised by the separate endpoint-aware classify lane.
        if *expected == "UNKNOWN" && matches!(*name, "docker" | "kubectl") {
            let shell = classify_shell(&f, name, args, env);
            assert_eq!(
                shell["classification"], "UNKNOWN",
                "{name} {args:?}: {shell}"
            );
            assert_eq!(shell["uncertainty"], "OBSERVE");
            shell_unknown += 1;
        }
        match *expected {
            "REMOTE_EXEC" => {
                remote += 1;
                if job["classification"]["locality"] == "HOST_LOCAL" {
                    false_local += 1;
                }
            }
            "UNKNOWN" => unknown += 1,
            "BUILD" => local += 1,
            _ => unreachable!(),
        }
    }
    assert_eq!(
        (
            cases.len(),
            remote,
            unknown,
            local,
            false_local,
            shell_unknown
        ),
        (27, 10, 17, 0, 0, 14)
    );
    eprintln!("offline exec fixtures: 27 total, 10 remote recognized, 17 unknown, 0 false-local among remote controls; 14 unknown Docker and Kubernetes controls also checked via classify");
    assert!(!fs::read(f.root.join("interface-calls")).unwrap().is_empty());
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[path = "capture_endpoints/held.rs"]
mod held;
