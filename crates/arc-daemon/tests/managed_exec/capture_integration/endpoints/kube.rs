use super::*;

pub(super) fn kube_selection_is_observed_in_both_cli_routes_without_inferring_placement() {
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
    let mut daemon = f.spawn_isolated("test");

    struct Selection<'a> {
        name: &'static str,
        args: Vec<&'a str>,
        config: Option<&'a str>,
        expected: Option<(
            &'static str,
            &'static str,
            &'static str,
            &'static str,
            &'static str,
        )>,
    }
    let cases = [
        Selection {
            name: "current",
            args: vec!["--kubeconfig", kube],
            config: None,
            expected: Some((
                "@current",
                "https://192.0.2.41:6443",
                "first",
                "alpha",
                "default",
            )),
        },
        Selection {
            name: "explicit-context",
            args: vec!["--kubeconfig", kube, "--context", "second"],
            config: None,
            expected: Some((
                "second",
                "https://192.0.2.42:6443",
                "second",
                "beta",
                "default",
            )),
        },
        Selection {
            name: "explicit-namespace",
            args: vec![
                "--kubeconfig",
                kube,
                "--context",
                "second",
                "--namespace",
                "builds",
            ],
            config: None,
            expected: Some((
                "second",
                "https://192.0.2.42:6443",
                "second",
                "beta",
                "builds",
            )),
        },
        Selection {
            name: "environment",
            args: vec![],
            config: Some(alternate),
            expected: Some((
                "@current",
                "https://192.0.2.43:6443",
                "first",
                "gamma",
                "default",
            )),
        },
        Selection {
            name: "first-config-wins",
            args: vec![],
            config: Some(first_list),
            expected: Some((
                "@current",
                "https://192.0.2.41:6443",
                "first",
                "alpha",
                "default",
            )),
        },
        Selection {
            name: "reversed-config-wins",
            args: vec![],
            config: Some(reversed_list),
            expected: Some((
                "@current",
                "https://192.0.2.43:6443",
                "first",
                "gamma",
                "default",
            )),
        },
        Selection {
            name: "flag-over-environment",
            args: vec!["--kubeconfig", kube, "--context", "second"],
            config: Some(alternate),
            expected: Some((
                "second",
                "https://192.0.2.42:6443",
                "second",
                "beta",
                "default",
            )),
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
    let receive = |route: &str, expected: Option<(&str, &str, &str, &str, &str)>, name: &str| {
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
            Some((selector, endpoint, selected_context, cluster, namespace)) => {
                assert_eq!(kube_facts.len(), 4, "{name}: {context}");
                assert_eq!(kube_facts[0]["selector"], selector, "{name}");
                assert_eq!(kube_facts[0]["endpoints"], json!([endpoint]), "{name}");
                for fact in &kube_facts[1..] {
                    assert_eq!(fact["endpoints"], json!([endpoint]), "{name}");
                }
                assert_eq!(
                    kube_facts[1]["selector"],
                    format!("@selected-context/{selected_context}"),
                    "{name}"
                );
                assert_eq!(
                    kube_facts[2]["selector"],
                    format!("@selected-cluster/{cluster}"),
                    "{name}"
                );
                assert_eq!(
                    kube_facts[3]["selector"],
                    format!("@selected-namespace/{namespace}"),
                    "{name}"
                );
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
        Some((
            "@current",
            "https://192.0.2.41:6443",
            "first",
            "alpha",
            "default",
        )),
        "multi-leaf",
    );
    receive("shell", None, "multi-leaf");
    daemon.stop(libc::SIGTERM);
    f.report();
}

pub(super) fn kube_api_endpoints_are_not_persisted_by_either_cli_route() {
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
    let mut daemon = f.spawn_isolated("test");
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
