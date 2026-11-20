use super::*;
use crate::{analyze_argv_with_context, MAX_WORK};
use std::ffi::OsString;

#[test]
fn remote_addresses_require_distinct_complete_local_inventory() {
    let context = EndpointContext {
        captured_at_ms: 10_000,
        facts: vec![EndpointFact {
            kind: EndpointKind::SshDestination,
            selector: "@local-addresses".into(),
            endpoints: vec!["192.0.2.5".into(), "::1".into()],
        }],
        ..EndpointContext::default()
    };
    for value in [
        "127.0.0.1",
        "::1",
        "::ffff:127.0.0.1",
        "::127.0.0.1",
        "0.0.0.0",
        "169.254.1.1",
        "224.0.0.1",
        "192.0.2.5",
    ] {
        assert!(!remote_ip(value.parse().unwrap(), &context), "{value}");
    }
    assert!(remote_ip("192.0.2.30".parse().unwrap(), &context));
    assert!(!remote_ip(
        "192.0.2.30".parse().unwrap(),
        &EndpointContext::default()
    ));
    for value in [
        "unix:///tmp/forwarded-docker.sock",
        "unix:///var/run/docker.sock",
        "ssh://192.0.2.30:22",
        "tcp://localhost:2376",
        "tcp://192.0.2.30:0",
        "tcp://127.0.0.1:2376",
    ] {
        assert!(place(value, &context).is_none(), "{value}");
    }
}

#[test]
fn docker_unix_socket_alone_never_proves_local_compute() {
    let context = EndpointContext {
        captured_at_ms: 10_000,
        facts: [EndpointKind::DockerContext, EndpointKind::DockerBuilder]
            .into_iter()
            .map(|kind| EndpointFact {
                kind,
                selector: "local".into(),
                endpoints: vec!["unix:///tmp/fixture-docker.sock".into()],
            })
            .collect(),
        ..EndpointContext::default()
    };
    let argv = ["docker", "--context", "local", "build", "."].map(OsString::from);
    let request = crate::ClassifyRequest {
        command: "docker --context local build .".into(),
        shell_kind: "sh".into(),
    };
    for now in [10_000, 15_001] {
        let argv_result = crate::classify_argv_with_context(&argv, &context, now);
        let shell_result = crate::classify_with_context(&request, &context, now);
        for result in [argv_result, shell_result] {
            assert_eq!(result.classification, JobClass::Unknown);
            assert_eq!(result.locality, crate::Locality::Unknown);
            assert_eq!(result.reason, Reason::EndpointAmbiguous);
            assert_eq!(result.uncertainty, crate::Uncertainty::Observe);
        }
    }
}

#[test]
fn endpoint_scans_obey_the_existing_work_limit() {
    let context = EndpointContext {
        captured_at_ms: 10_000,
        ..EndpointContext::default()
    };
    let args = vec![
        OsString::from("ssh"),
        OsString::from("x".repeat(16_000)),
        OsString::from("make"),
    ];
    let (result, stats) = analyze_argv_with_context(&args, &context, 10_000);
    assert_eq!(result.classification, JobClass::Unknown);
    assert!(stats.work <= MAX_WORK);
}
