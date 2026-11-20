use super::*;

pub(super) fn install(s: &Sandbox) {
    write(
        &s.home.join("gh/hosts.yml"),
        "github.com:\n  user: fixture\n",
    );
    write(
        &s.home.join(".docker/config.json"),
        r#"{"currentContext":"remote"}"#,
    );
    for (name, hash, endpoint) in [
        (
            "remote",
            "b71199ebd070b36beab7317920c2c2f1d777df8d05e5527d8458fda57cb17a7a",
            "tcp://192.0.2.20:2376",
        ),
        (
            "local",
            "25bf8e1a2393f1108d37029b3df5593236c755742ec93465bbafa9b290bddcf6",
            "unix:///tmp/arc-fixture.sock",
        ),
    ] {
        write(
            &s.home
                .join(".docker/contexts/meta")
                .join(hash)
                .join("meta.json"),
            json!({"Name":name,"Endpoints":{"docker":{"Host":endpoint}}}).to_string(),
        );
    }
    for (name, driver, nodes) in [
        (
            "remote-builder",
            "remote",
            json!([{"Name":"node0","Endpoint":"tcp://192.0.2.21:1234"}]),
        ),
        (
            "local-builder",
            "docker-container",
            json!([{"Name":"node0","Endpoint":"local"}]),
        ),
        (
            "mixed-builder",
            "remote",
            json!([{"Name":"node0","Endpoint":"tcp://192.0.2.21:1234"},{"Name":"node1","Endpoint":"unix:///tmp/arc-fixture.sock"}]),
        ),
    ] {
        write(
            &s.home.join(".docker/buildx/instances").join(name),
            json!({"Name":name,"Driver":driver,"Dynamic":false,"Nodes":nodes}).to_string(),
        );
    }
    write(
        &s.home.join(".docker/buildx/defaults/b71199ebd070b36beab7"),
        "remote-builder",
    );
    write(&s.home.join("ssh.conf"), "Host worker\n HostName 192.0.2.24\n Port 2222\n ProxyJump none\nHost *\n HostName 192.0.2.25\n Port 22\n");
    write(
        &s.home.join("ssh-proxy.conf"),
        "Host worker\n HostName 192.0.2.24\n ProxyJump jump\n",
    );
    write(
        &s.home.join("ssh-local.conf"),
        "Host worker\n HostName 127.0.0.1\nHost synthetic-interface\n HostName 192.0.2.5\n",
    );
    write(&s.home.join("kube.yaml"), "current-context: first\ncontexts:\n- name: first\n  context:\n    cluster: alpha\n- name: second\n  context:\n    cluster: beta\nclusters:\n- name: alpha\n  cluster:\n    server: https://192.0.2.41:6443\n- name: beta\n  cluster:\n    server: https://192.0.2.42:6443\n");
}

struct Case<'a> {
    name: &'static str,
    argv: Vec<&'a str>,
    env: Vec<(&'a str, Option<&'a str>)>,
    config: &'static str,
    expected: &'static str,
}

const REMOTE_GIT: &str = "[remote \"origin\"]\n url = https://github.com/example/app.git\n";
const LOCAL_GIT: &str = "[remote \"origin\"]\n url = file:///fixture/local\n";
const AMBIGUOUS_GIT: &str = "[remote \"origin\"]\n url = https://github.com/example/app.git\n pushurl = https://github.com/example/app.git\n pushurl = file:///fixture/local\n";

#[test]
fn both_routes_use_offline_source_selection_and_conservative_uncertainty() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    install(&s);
    let mut daemon = f.spawn("test");
    let ssh = s.home.join("ssh.conf");
    let proxy = s.home.join("ssh-proxy.conf");
    let local = s.home.join("ssh-local.conf");
    let kube = s.home.join("kube.yaml");
    let missing = s.home.join("missing");
    let ssh = ssh.to_str().unwrap();
    let proxy = proxy.to_str().unwrap();
    let local = local.to_str().unwrap();
    let kube = kube.to_str().unwrap();
    let missing = missing.to_str().unwrap();
    macro_rules! case {
        ($name:literal, [$($arg:expr),+], [$($key:literal => $value:expr),*], $config:expr, $expected:literal) => {
            Case { name:$name, argv:vec![$($arg),+], env:vec![$(($key, Some($value))),*], config:$config, expected:$expected }
        };
    }
    #[rustfmt::skip]
    let cases = [
        case!("ssh_config", ["ssh", "-F", ssh, "worker", "make"], [], "", "REMOTE_EXEC"),
        case!("ssh_flag_port", ["ssh", "-F", ssh, "-p", "2299", "worker", "make"], [], "", "REMOTE_EXEC"),
        case!("ssh_proxy", ["ssh", "-F", proxy, "worker", "make"], [], "", "UNKNOWN"),
        case!("ssh_missing", ["ssh", "-F", missing, "worker", "make"], [], "", "UNKNOWN"),
        case!("ssh_loopback", ["ssh", "-F", local, "worker", "make"], [], "", "UNKNOWN"),
        case!("ssh_synthetic_local_interface", ["ssh", "-F", local, "synthetic-interface", "make"], [], "", "UNKNOWN"),
        case!("docker_current_context", ["docker", "build", "."], [], "", "REMOTE_EXEC"),
        case!("docker_cli_context_before_env", ["docker", "--context", "remote", "build", "."], ["DOCKER_HOST" => "unix:///tmp/fixture.sock"], "", "REMOTE_EXEC"),
        case!("docker_host_before_context_env", ["docker", "build", "."], ["DOCKER_CONTEXT" => "local", "DOCKER_HOST" => "tcp://192.0.2.20:2376"], "", "REMOTE_EXEC"),
        case!("docker_local_socket", ["docker", "--context", "local", "build", "."], [], "", "UNKNOWN"),
        case!("docker_missing_context", ["docker", "--context", "missing", "build", "."], [], "", "UNKNOWN"),
        case!("docker_conflicting_selectors", ["docker", "--context", "remote", "--host", "tcp://192.0.2.20:2376", "build", "."], [], "", "UNKNOWN"),
        case!("buildx_selected_builder", ["docker", "buildx", "build", "--builder", "remote-builder", "."], ["BUILDX_BUILDER" => "local-builder"], "", "REMOTE_EXEC"),
        case!("buildx_remote_over_local_context", ["docker", "--context", "local", "buildx", "--builder", "remote-builder", "build", "."], [], "", "REMOTE_EXEC"),
        case!("buildx_default", ["docker", "buildx", "build", "."], [], "", "REMOTE_EXEC"),
        case!("buildx_mixed_nodes", ["docker", "buildx", "build", "--builder", "mixed-builder", "."], [], "", "UNKNOWN"),
        case!("buildx_missing", ["docker", "buildx", "build", "--builder", "missing", "."], [], "", "UNKNOWN"),
        case!("kube_context_no_placement", ["kubectl", "--kubeconfig", kube, "--context", "second", "exec", "pod", "--", "make"], [], "", "UNKNOWN"),
        case!("kube_missing", ["kubectl", "--kubeconfig", missing, "exec", "pod", "--", "make"], [], "", "UNKNOWN"),
        case!("kube_cluster_override", ["kubectl", "--kubeconfig", kube, "--cluster", "beta", "exec", "pod", "--", "make"], [], "", "UNKNOWN"),
        case!("kube_server_override", ["kubectl", "--kubeconfig", kube, "--server=https://192.0.2.43:6443", "exec", "pod", "--", "make"], [], "", "UNKNOWN"),
        case!("kube_conflicting_contexts", ["kubectl", "--kubeconfig", kube, "--context", "first", "--context", "second", "exec", "pod", "--", "make"], [], "", "UNKNOWN"),
        case!("gh_repository", ["gh", "workflow", "run", "build.yml"], [], REMOTE_GIT, "REMOTE_CI"),
        case!("gh_explicit_repo_over_host", ["gh", "workflow", "run", "build.yml", "--repo", "github.com/example/app"], ["GH_HOST" => "enterprise.invalid"], "", "REMOTE_CI"),
        case!("gh_conflicting_host", ["gh", "workflow", "run", "build.yml"], ["GH_HOST" => "enterprise.invalid"], REMOTE_GIT, "UNKNOWN"),
        case!("gh_missing_repository", ["gh", "workflow", "run", "build.yml"], [], "", "UNKNOWN"),
        case!("bare_remote_origin_fallback", ["git", "push"], [], REMOTE_GIT, "REMOTE_CI"),
        case!("bare_local_origin_fallback", ["git", "push"], [], LOCAL_GIT, "UNKNOWN"),
        case!("bare_ambiguous_pushurls", ["git", "push"], [], AMBIGUOUS_GIT, "UNKNOWN"),
        case!("explicit_git_remote", ["git", "push", "origin", "topic"], [], REMOTE_GIT, "REMOTE_CI"),
        case!("explicit_git_missing", ["git", "push", "origin", "topic"], [], "", "UNKNOWN"),
    ];
    let (mut remote, mut unknown, mut reason_differences) = (0, 0, 0);
    for case in &cases {
        write(&s.repo.join(".git/config"), case.config);
        let interface_calls = fs::read(s.home.join("interface-calls"))
            .unwrap_or_default()
            .len();
        let inspection = s.classify(&f, &case.argv, &case.env);
        let (execution, _) = s.exec(&f, &case.argv, &case.env);
        if case.name == "ssh_synthetic_local_interface" {
            let after = fs::read(s.home.join("interface-calls")).unwrap().len();
            assert!(
                after >= interface_calls + 2,
                "both CLI routes must use the synthetic interface inventory"
            );
        }
        reason_differences += usize::from(assert_routes(
            case.name,
            &inspection,
            &execution,
            case.expected,
        ));
        if case.expected == "UNKNOWN" {
            unknown += 1;
        } else {
            remote += 1;
        }
    }
    assert_eq!((cases.len(), remote, unknown), (31, 12, 19));
    assert_eq!(s.calls(), cases.len());
    eprintln!("combined source fixtures: {} unique cases, {remote} remote, {unknown} UNKNOWN, zero false-local per route, {reason_differences} UNKNOWN reason differences", cases.len());
    assert_eq!(
        reason_differences, 0,
        "source classification reasons differ"
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn kube_selection_is_observed_in_both_cli_routes_without_inferring_placement() {
    use std::{os::unix::net::UnixDatagram, time::Duration};
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    install(&s);
    let alternate = s.home.join("alternate-kube.yaml");
    write(&alternate, "current-context: first\ncontexts:\n- name: first\n  context:\n    cluster: gamma\n- name: second\n  context:\n    cluster: gamma\nclusters:\n- name: gamma\n  cluster:\n    server: https://192.0.2.43:6443\n");
    let kube = s.home.join("kube.yaml");
    let missing = s.home.join("missing-kube.yaml");
    let kube = kube.to_str().unwrap();
    let alternate = alternate.to_str().unwrap();
    let missing = missing.to_str().unwrap();
    let first_list = std::env::join_paths([kube, alternate]).unwrap();
    let reversed_list = std::env::join_paths([alternate, kube]).unwrap();
    let first_list = first_list.to_str().unwrap();
    let reversed_list = reversed_list.to_str().unwrap();
    let socket_path = f.root.join("capture.sock");
    let socket = UnixDatagram::bind(&socket_path).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let socket_path = socket_path.to_str().unwrap();
    let mut daemon = f.spawn("test");

    struct Selection<'a> {
        name: &'static str,
        args: Vec<&'a str>,
        config: Option<&'a str>,
        expected: Option<(&'static str, &'static str)>,
    }
    let cases = [
        Selection {
            name: "current",
            args: vec!["--kubeconfig", kube],
            config: None,
            expected: Some(("@current", "https://192.0.2.41:6443")),
        },
        Selection {
            name: "explicit-context",
            args: vec!["--kubeconfig", kube, "--context", "second"],
            config: None,
            expected: Some(("second", "https://192.0.2.42:6443")),
        },
        Selection {
            name: "environment",
            args: vec![],
            config: Some(alternate),
            expected: Some(("@current", "https://192.0.2.43:6443")),
        },
        Selection {
            name: "first-config-wins",
            args: vec![],
            config: Some(first_list),
            expected: Some(("@current", "https://192.0.2.41:6443")),
        },
        Selection {
            name: "reversed-config-wins",
            args: vec![],
            config: Some(reversed_list),
            expected: Some(("@current", "https://192.0.2.43:6443")),
        },
        Selection {
            name: "flag-over-environment",
            args: vec!["--kubeconfig", kube, "--context", "second"],
            config: Some(alternate),
            expected: Some(("second", "https://192.0.2.42:6443")),
        },
        Selection {
            name: "missing",
            args: vec!["--kubeconfig", missing],
            config: Some(alternate),
            expected: None,
        },
        Selection {
            name: "conflicting-context",
            args: vec![
                "--kubeconfig",
                kube,
                "--context",
                "first",
                "--context",
                "second",
            ],
            config: None,
            expected: None,
        },
        Selection {
            name: "unsupported-server-override",
            args: vec!["--kubeconfig", kube, "--server=https://192.0.2.44:6443"],
            config: None,
            expected: None,
        },
    ];
    let receive = |route: &str, expected: Option<(&str, &str)>, name: &str| {
        let mut bytes = vec![0; 32 * 1024];
        let length = socket
            .recv(&mut bytes)
            .expect("capture observation watchdog");
        let delimiter = bytes[..length]
            .iter()
            .position(|byte| *byte == b'\t')
            .unwrap();
        let actual_route = &bytes[..delimiter];
        let payload = &bytes[delimiter + 1..length];
        assert_eq!(actual_route, route.as_bytes(), "{name}");
        let context: Value = serde_json::from_slice(payload).unwrap();
        if context.is_null() {
            assert!(expected.is_none(), "{name}: shell capture was discarded");
            return;
        }
        let facts = context["facts"].as_array().unwrap();
        let kube_facts: Vec<_> = facts
            .iter()
            .filter(|fact| fact["kind"] == "KUBERNETES_CONTEXT")
            .collect();
        match expected {
            Some((selector, endpoint)) => {
                assert_eq!(kube_facts.len(), 1, "{name}: {context}");
                assert_eq!(kube_facts[0]["selector"], selector, "{name}");
                assert_eq!(kube_facts[0]["endpoints"], json!([endpoint]), "{name}");
            }
            None => assert!(kube_facts.is_empty(), "{name}: {context}"),
        }
        assert!(facts.iter().all(|fact| fact["kind"] != "KUBERNETES_POD"));
    };
    for case in cases {
        let mut argv = vec!["kubectl"];
        argv.extend(case.args);
        argv.extend(["exec", "pod", "--", "make"]);
        let env = [
            ("KUBECONFIG", case.config),
            ("ARC_TEST_CAPTURE_SOCKET", Some(socket_path)),
        ];
        let inspection = s.classify(&f, &argv, &env);
        receive("argv", case.expected, case.name);
        receive("shell", case.expected, case.name);
        let (execution, _) = s.exec(&f, &argv, &env);
        receive("argv", case.expected, case.name);
        assert!(!assert_routes(
            case.name,
            &inspection,
            &execution,
            "UNKNOWN"
        ));
    }
    let shell = format!(
        "kubectl --kubeconfig '{kube}' exec pod -- make && kubectl --kubeconfig '{kube}' --context second exec pod -- make"
    );
    let mut command = s.command(&f, &[("ARC_TEST_CAPTURE_SOCKET", Some(socket_path))]);
    command.args(["classify", "--json", "--", &shell]);
    let output = child_watchdog::output(&mut command, "multi-leaf kube classify").unwrap();
    assert!(output.status.success(), "{output:?}");
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["classification"], "UNKNOWN");
    assert_eq!(result["uncertainty"], "OBSERVE");
    receive(
        "argv",
        Some(("@current", "https://192.0.2.41:6443")),
        "multi-leaf",
    );
    receive("shell", None, "multi-leaf");
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn kube_api_endpoints_are_not_persisted_by_either_cli_route() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    install(&s);
    let servers = ["https://192.0.2.41:6443", "https://192.0.2.42:6443"];
    let kube = s.home.join("kube.yaml");
    let fixture = fs::read(&kube).unwrap();
    for server in servers {
        assert!(fixture
            .windows(server.len())
            .any(|bytes| bytes == server.as_bytes()));
    }
    let mut daemon = f.spawn("test");
    let argv = [
        "kubectl",
        "--kubeconfig",
        kube.to_str().unwrap(),
        "--context",
        "second",
        "exec",
        "pod",
        "--",
        "make",
    ];
    let inspection = s.classify(&f, &argv, &[]);
    let (execution, _) = s.exec(&f, &argv, &[]);
    assert!(!assert_routes(
        "kube nonpersistence",
        &inspection,
        &execution,
        "UNKNOWN"
    ));
    for response in [&inspection, &execution] {
        let bytes = serde_json::to_vec(response).unwrap();
        for server in servers {
            assert!(!bytes
                .windows(server.len())
                .any(|window| window == server.as_bytes()));
        }
    }
    let assert_daemon_files = || {
        for name in [
            "arc.sqlite3",
            "arc.sqlite3-wal",
            "arc.sqlite3-shm",
            "stdout",
            "stderr",
            "runtime.json",
        ] {
            let path = f.root.join(name);
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => panic!("cannot read daemon artifact {name}: {error}"),
            };
            for server in servers {
                assert!(
                    !bytes
                        .windows(server.len())
                        .any(|window| window == server.as_bytes()),
                    "fixture API endpoint persisted in {name}"
                );
            }
        }
    };
    assert_daemon_files();
    daemon.stop(libc::SIGTERM);
    f.report();
    assert_daemon_files();
}
