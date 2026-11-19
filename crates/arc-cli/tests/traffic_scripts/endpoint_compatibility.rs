use super::execution::{command, setup, unsupported};
use crate::support::*;
use arc_ipc::{governor::*, traffic};
use std::{fs, thread};

#[test]
fn unverified_endpoints_and_unsupported_graphs_execute_as_observations_without_local_admission() {
    for body in [
        "gh workflow run ci.yml",
        "ssh remote.example 'bun run build'",
        "vite build --watch",
        "vite build && biome check .",
        "bun run surprise",
    ] {
        let socket = Socket::new();
        setup(&socket, body);
        let listener = socket.listener.try_clone().unwrap();
        let server = thread::spawn(move || {
            for _ in 0..3 {
                let (stream, request) = next(&listener);
                assert_eq!(
                    request.operation,
                    arc_ipc::jobs::OPERATION_JOB_START,
                    "Bypass graphs must never enqueue or negotiate a reservation"
                );
                assert_eq!(request.payload["classification"]["locality"], "UNKNOWN");
                assert_eq!(
                    request.payload["classification"]["classification"],
                    "UNKNOWN"
                );
                assert_eq!(request.payload["classification"]["uncertainty"], "OBSERVE");
                drop(stream);
            }
        });
        let mut invocation = command(&socket);
        invocation
            .env("GH_HOST", "github.com")
            .env_remove("GH_REPO");
        let result = output(invocation);
        server.join().unwrap();
        assert_eq!(result.status.code(), Some(17));
        assert_eq!(
            fs::read_to_string(socket.root.join("starts"))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }
}

#[test]
fn unrelated_invalid_endpoint_environment_does_not_bypass_local_admission() {
    use std::ffi::OsString;
    use std::os::unix::{ffi::OsStringExt, fs::symlink};

    for value in [
        OsString::from("x".repeat(1_025)),
        OsString::from_vec(vec![0xff]),
    ] {
        for (program, args, local) in [
            ("cargo", vec!["build"], true),
            ("kubectl", vec!["exec", "remote-pod", "--", "make"], false),
        ] {
            let socket = Socket::new();
            symlink(
                env!("CARGO_BIN_EXE_arc-exec-fixture"),
                socket.root.join(program),
            )
            .unwrap();
            let listener = socket.listener.try_clone().unwrap();
            let server = thread::spawn(move || {
                if local {
                    let (mut stream, request) = next(&listener);
                    assert_eq!(request.operation, OPERATION_CAPABILITIES);
                    unsupported(&mut stream, &request);
                    let (mut stream, request) = next(&listener);
                    assert_eq!(request.operation, traffic::OPERATION_TRAFFIC_CAPABILITIES);
                    unsupported(&mut stream, &request);
                }
                for _ in 0..3 {
                    let (stream, request) = next(&listener);
                    assert_eq!(request.operation, arc_ipc::jobs::OPERATION_JOB_START);
                    let classification = &request.payload["classification"];
                    if local {
                        assert_eq!(classification["classification"], "BUILD");
                        assert_eq!(classification["locality"], "HOST_LOCAL");
                        assert_eq!(classification["reason"], "CARGO_BUILD");
                    } else {
                        assert_eq!(classification["classification"], "UNKNOWN");
                        assert_eq!(classification["locality"], "UNKNOWN");
                        assert_eq!(classification["uncertainty"], "OBSERVE");
                    }
                    drop(stream);
                }
            });
            let mut invocation = socket.cli();
            invocation
                .args(["exec", "--", program])
                .args(args)
                .env("PATH", &socket.root)
                .env("KUBECONFIG", &value)
                .env("ARC_FIXTURE_MODE", "exit17")
                .env("ARC_FIXTURE_MARKER", socket.root.join("starts"));
            let result = output(invocation);
            server.join().unwrap();
            assert_eq!(result.status.code(), Some(17));
            assert_eq!(
                fs::read_to_string(socket.root.join("starts"))
                    .unwrap()
                    .lines()
                    .count(),
                1
            );
        }
    }
}

#[test]
fn invalid_endpoint_environment_preserves_only_local_package_script_admission() {
    for (body, local) in [("vite build", true), ("gh workflow run ci.yml", false)] {
        let socket = Socket::new();
        setup(&socket, body);
        let listener = socket.listener.try_clone().unwrap();
        let server = thread::spawn(move || {
            if local {
                let (mut stream, request) = next(&listener);
                assert_eq!(
                    request.operation,
                    traffic::OPERATION_BUN_SCRIPTS_CAPABILITIES
                );
                unsupported(&mut stream, &request);
            }
            for _ in 0..3 {
                let (stream, request) = next(&listener);
                assert_eq!(request.operation, arc_ipc::jobs::OPERATION_JOB_START);
                // Local script vocabulary is projected for this old peer.
                assert_eq!(
                    request.payload["classification"]["classification"],
                    "UNKNOWN"
                );
                drop(stream);
            }
        });
        let mut invocation = command(&socket);
        invocation
            .env("KUBECONFIG", "x".repeat(1_025))
            .env("GH_HOST", "github.com")
            .env_remove("GH_REPO");
        let result = output(invocation);
        server.join().unwrap();
        assert_eq!(result.status.code(), Some(17));
        assert_eq!(
            fs::read_to_string(socket.root.join("starts"))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }
}
