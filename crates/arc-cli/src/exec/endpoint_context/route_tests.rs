//! Subprocess isolation supplies a temporary HOME without changing test-global env.
use std::{fs, process::Command};
#[path = "../../../../arc-daemon/tests/support/child_watchdog.rs"]
#[allow(clippy::duplicate_mod)] // The standalone test binary reuses this watchdog.
mod child_watchdog;

#[test]
fn captured_shell_and_argv_share_endpoint_facts() {
    if std::env::var_os("ARC_CAPTURE_ROUTE_CHILD").is_some() {
        assert_eq!(
            std::env::var_os("LLVM_PROFILE_FILE"),
            std::env::var_os("ARC_CAPTURE_EXPECT_PROFILE"),
            "isolated child lost the coverage destination"
        );
        let home = std::env::var("HOME").unwrap();
        assert!(!home.contains('\''));
        for (command, expected) in [
            (
                "docker build .".to_owned(),
                arc_detector::JobClass::RemoteExec,
            ),
            (
                "docker buildx build --builder fixture-builder .".to_owned(),
                arc_detector::JobClass::RemoteExec,
            ),
            (
                format!("ssh -F '{home}/ssh.conf' worker make"),
                arc_detector::JobClass::RemoteExec,
            ),
            (
                format!("kubectl --kubeconfig '{home}/kube.yaml' exec pod -- make"),
                arc_detector::JobClass::Unknown,
            ),
        ] {
            let request = arc_detector::ClassifyRequest {
                shell_kind: "sh".into(),
                command,
            };
            let argv = arc_detector::capture_shell_argv(&request)
                .unwrap()
                .remove(0);
            let argv_context = super::super::capture_argv(&argv).unwrap();
            let shell_context = super::super::capture_shell(&request);
            if request.command.starts_with("docker") || request.command.starts_with("ssh") {
                for context in [&argv_context, shell_context.as_ref().unwrap()] {
                    let inventory = context
                        .facts
                        .iter()
                        .find(|fact| {
                            fact.kind == arc_detector::EndpointKind::SshDestination
                                && fact.selector == "@local-addresses"
                        })
                        .expect("synthetic interface inventory missing");
                    assert_eq!(inventory.endpoints, ["192.0.2.5"]);
                }
            }
            let now = arc_ipc::locality::now_ms().unwrap();
            let direct = arc_detector::classify_argv_with_context(&argv, &argv_context, now);
            assert_eq!(direct.classification, expected);
            if expected == arc_detector::JobClass::Unknown {
                let shell_context = shell_context.unwrap();
                for context in [&argv_context, &shell_context] {
                    let facts: Vec<_> = context
                        .facts
                        .iter()
                        .filter(|fact| fact.kind == arc_detector::EndpointKind::KubernetesContext)
                        .collect();
                    assert_eq!(facts.len(), 1);
                    assert_eq!(facts[0].selector, "@current");
                    assert_eq!(facts[0].endpoints, ["https://192.0.2.41:6443"]);
                    assert!(!context
                        .facts
                        .iter()
                        .any(|fact| fact.kind == arc_detector::EndpointKind::KubernetesPod));
                }
                let shell = arc_detector::classify_with_context(&request, &shell_context, now);
                assert_eq!(shell, direct);
                assert_eq!(shell.locality, arc_detector::Locality::Unknown);
                assert_eq!(shell.uncertainty, arc_detector::Uncertainty::Observe);

                let missing =
                    format!("kubectl --kubeconfig '{home}/missing-kube.yaml' exec other -- make");
                for (command, expected_facts) in [
                    (format!("{} && {missing}", request.command), [true, false]),
                    (format!("{missing} && {}", request.command), [false, true]),
                ] {
                    let compound = arc_detector::ClassifyRequest {
                        shell_kind: "sh".into(),
                        command,
                    };
                    let leaves = arc_detector::capture_shell_argv(&compound).unwrap();
                    assert_eq!(leaves.len(), 2);
                    for (leaf, expected_fact) in leaves.iter().zip(expected_facts) {
                        let context = super::super::capture_argv(leaf).unwrap();
                        assert_eq!(
                            context.facts.iter().any(|fact| {
                                fact.kind == arc_detector::EndpointKind::KubernetesContext
                            }),
                            expected_fact
                        );
                    }
                    assert!(super::super::capture_shell(&compound).is_none());
                }
            } else {
                let shell =
                    arc_detector::classify_with_context(&request, &shell_context.unwrap(), now);
                assert_eq!(shell, direct);
            }
        }
        let ambiguous = "docker --context missing --host tcp://192.0.2.20:2376 build .";
        let request = arc_detector::ClassifyRequest {
            shell_kind: "sh".into(),
            command: ambiguous.into(),
        };
        let argv = arc_detector::capture_shell_argv(&request)
            .unwrap()
            .remove(0);
        let direct_context = super::super::capture_argv(&argv).unwrap();
        let shell_context = super::super::capture_shell(&request).unwrap();
        let now = arc_ipc::locality::now_ms().unwrap();
        let direct = arc_detector::classify_argv_with_context(&argv, &direct_context, now);
        let shell = arc_detector::classify_with_context(&request, &shell_context, now);
        assert_eq!(direct.classification, arc_detector::JobClass::Unknown);
        assert_eq!(direct.locality, arc_detector::Locality::Unknown);
        assert_eq!(direct.uncertainty, arc_detector::Uncertainty::Observe);
        assert_eq!(shell, direct);
        for command in [
            format!("{ambiguous} && docker build ."),
            format!("docker build . && {ambiguous}"),
        ] {
            let compound = arc_detector::ClassifyRequest {
                shell_kind: "sh".into(),
                command,
            };
            assert!(super::super::capture_shell(&compound).is_none());
        }
        return;
    }
    let root = std::env::temp_dir().join(format!("arc-route-capture-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    fs::create_dir_all(root.join(".docker/buildx/instances")).unwrap();
    fs::write(root.join(".docker/buildx/instances/fixture-builder"),
        r#"{"Name":"fixture-builder","Driver":"remote","Dynamic":false,"Nodes":[{"Name":"node","Endpoint":"tcp://192.0.2.21:1234"}]}"#).unwrap();
    fs::write(root.join("ssh.conf"), "Host worker\n HostName 192.0.2.24\n").unwrap();
    fs::write(root.join("kube.yaml"), "current-context: one\ncontexts:\n- name: one\n  context:\n    cluster: one\nclusters:\n- name: one\n  cluster:\n    server: https://192.0.2.41:6443\n").unwrap();
    let source = root.join("interfaces.c");
    let library = root.join("interfaces-library");
    let marker = root.join("interface-calls");
    fs::write(
        &source,
        include_str!("../../../../arc-daemon/tests/managed_exec/capture_integration/interfaces.c"),
    )
    .unwrap();
    let mut compiler = Command::new("/usr/bin/cc");
    compiler.env_clear().env("PATH", "/usr/bin:/bin");
    if cfg!(target_os = "macos") {
        compiler.arg("-dynamiclib");
    } else {
        compiler.args(["-shared", "-fPIC"]);
    }
    compiler.arg("-o").arg(&library).arg(&source);
    let output = child_watchdog::output(&mut compiler, "fixture interface compiler").unwrap();
    assert!(
        output.status.success(),
        "fixture compiler failed: {output:?}"
    );
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .env("HOME", root)
        .env("DOCKER_HOST", "tcp://192.0.2.20:2376")
        .env("ARC_CAPTURE_ROUTE_CHILD", "1")
        .env("ARC_TEST_INTERFACE_MARKER", &marker)
        .env(
            if cfg!(target_os = "macos") {
                "DYLD_INSERT_LIBRARIES"
            } else {
                "LD_PRELOAD"
            },
            &library,
        )
        .args([
            "--exact",
            "exec::endpoint_context::route_tests::captured_shell_and_argv_share_endpoint_facts",
            "--nocapture",
        ]);
    // An instrumented child otherwise writes a default profile in the source
    // tree after env_clear removes the runner's configured destination.
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command
            .env("LLVM_PROFILE_FILE", &profile)
            .env("ARC_CAPTURE_EXPECT_PROFILE", profile);
    }
    let output = child_watchdog::output(&mut command, "isolated endpoint shell capture").unwrap();
    assert!(output.status.success(), "child failed: {output:?}");
    assert!(
        fs::read(&marker).unwrap().len() >= 6,
        "both captures for Docker, Buildx and SSH must use the synthetic inventory"
    );
}
