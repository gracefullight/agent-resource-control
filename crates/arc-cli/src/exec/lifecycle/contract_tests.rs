use super::*;
use std::{
    fs,
    os::unix::{fs::PermissionsExt, io::AsRawFd, net::UnixListener},
    thread,
    time::Duration,
};

fn remote_classification() -> arc_ipc::Classification {
    let context: arc_detector::EndpointContext = serde_json::from_value(json!({
        "captured_at_ms": 10000,
        "facts": [
            {"kind":"DOCKER_CONTEXT", "selector":"prod", "endpoints":["tcp://192.0.2.40:2376"]},
            {"kind":"DOCKER_BUILDER", "selector":"prod", "endpoints":["tcp://192.0.2.40:2376"]},
            {"kind":"SSH_DESTINATION", "selector":"@local-addresses", "endpoints":["192.0.2.5"]}
        ]
    }))
    .unwrap();
    let argv = ["docker", "--context", "prod", "build", "."].map(std::ffi::OsString::from);
    let result = arc_detector::classify_argv_with_context(&argv, &context, 10000);
    assert_eq!(result.reason, arc_detector::Reason::RemoteEndpointExec);
    result
}

fn snapshot(request: &arc_ipc::Request) -> serde_json::Value {
    let p = &request.payload;
    json!({"disposition":"CREATED", "job":{
        "job_id":p["job_id"], "phase":"REGISTERED", "classification":p["classification"],
        "observer":p["observer"], "observer_verified_at_ms":1, "child":null,
        "child_verified_at_ms":null,
        "observation":{"target":"OBSERVER", "status":"VERIFIED_MATCH", "observed_at_ms":1, "reason":"REGISTERED_NOT_CLAIMED"},
        "reason":"REGISTERED_NOT_CLAIMED", "outcome":null, "registered_at_ms":1,
        "launch_claimed_at_ms":null, "completed_at_ms":null, "updated_at_ms":1, "revision":1
    }})
}

#[test]
fn contextual_registration_negotiates_once_and_never_falls_back_after_a_lost_ack() {
    for mode in ["supported", "unsupported", "lost"] {
        let root = std::env::temp_dir().join(format!(
            "arc-route-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let socket = root.join("s");
        let listener = UnixListener::bind(&socket).unwrap();
        let server = thread::spawn(move || {
            let next = || {
                let mut fd = libc::pollfd {
                    fd: listener.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                assert_eq!(
                    unsafe { libc::poll(&mut fd, 1, 60_000) },
                    1,
                    "registration request watchdog"
                );
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(60)))
                    .unwrap();
                let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
                (stream, request)
            };
            let (mut stream, first) = next();
            assert_eq!(first.operation, OPERATION_JOB_START_LOCALITY);
            assert_eq!(
                first.payload["classification"]["reason"],
                "REMOTE_ENDPOINT_EXEC"
            );
            if mode == "supported" {
                arc_ipc::write_response(
                    &mut stream,
                    &arc_ipc::Response::ok(&first.request_id, snapshot(&first)),
                )
                .unwrap();
            } else if mode == "unsupported" {
                arc_ipc::write_response(
                    &mut stream,
                    &arc_ipc::Response::error(
                        &first.request_id,
                        arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                    ),
                )
                .unwrap();
                let (mut stream, second) = next();
                assert_eq!(second.operation, OPERATION_JOB_START);
                assert_eq!(
                    second.payload["classification"]["classification"],
                    "UNKNOWN"
                );
                assert_eq!(second.payload["job_id"], first.payload["job_id"]);
                arc_ipc::write_response(
                    &mut stream,
                    &arc_ipc::Response::ok(&second.request_id, snapshot(&second)),
                )
                .unwrap();
            } else {
                drop(stream); // registration may have committed before its reply was lost
                let (mut stream, second) = next();
                assert_eq!(second.operation, OPERATION_JOB_START_LOCALITY);
                assert_eq!(second.request_id, first.request_id);
                arc_ipc::write_response(
                    &mut stream,
                    &arc_ipc::Response::error(
                        &second.request_id,
                        arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                    ),
                )
                .unwrap();
            }
        });
        let id = JobId::try_from("a".repeat(32)).unwrap();
        let input = StartRequest::Register {
            job_id: id,
            observer: ProcessIdentity::capture(std::process::id()).unwrap(),
            classification: remote_classification(),
            usage: None,
        };
        let outcome = start_until(
            &socket,
            input,
            Instant::now() + Duration::from_secs(10),
            &|| false,
        );
        match mode {
            "supported" => assert_eq!(
                outcome.unwrap().job.classification.reason,
                arc_detector::Reason::RemoteEndpointExec
            ),
            "unsupported" => assert_eq!(
                outcome.unwrap().job.classification,
                remote_classification().legacy()
            ),
            _ => assert!(matches!(outcome, Err(transport::Failure::Uncertain))),
        }
        server.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn proven_gh_and_ssh_keep_protocol_one_reasons_and_uncertainty_is_conservative() {
    use arc_detector::{JobClass, Locality, Reason, Uncertainty};

    let gh = json!({"captured_at_ms":10000,"facts":[
        {"kind":"CI_HOST","selector":"gh","endpoints":["https://github.com"]}
    ]});
    let gh_conflict = json!({"captured_at_ms":10000,"environment":{"GH_HOST":"localhost"},"facts":[
        {"kind":"CI_HOST","selector":"gh","endpoints":["https://github.com"]}
    ]});
    let ssh = json!({"captured_at_ms":10000,"facts":[
        {"kind":"SSH_DESTINATION","selector":"fixture","endpoints":["ssh://192.0.2.30:22"]},
        {"kind":"SSH_DESTINATION","selector":"@local-addresses","endpoints":["192.0.2.10"]}
    ]});
    let ssh_conflict = json!({"captured_at_ms":10000,"facts":[
        {"kind":"SSH_DESTINATION","selector":"fixture","endpoints":["ssh://192.0.2.30:22"]},
        {"kind":"SSH_DESTINATION","selector":"@local-addresses","endpoints":["192.0.2.30"]}
    ]});
    let missing = json!({"captured_at_ms":10000,"facts":[]});
    let gh_args = &["gh", "workflow", "run", "build.yml"][..];
    let ssh_args = &["ssh", "fixture", "make"][..];
    for (argv, context, now, reason) in [
        (gh_args, &gh, 10000, Reason::GhWorkflowRun),
        (ssh_args, &ssh, 10000, Reason::SshRemoteCommand),
        (gh_args, &missing, 10000, Reason::EndpointAmbiguous),
        (ssh_args, &missing, 10000, Reason::EndpointAmbiguous),
        (gh_args, &gh_conflict, 10000, Reason::EndpointAmbiguous),
        (ssh_args, &ssh_conflict, 10000, Reason::EndpointAmbiguous),
        (gh_args, &gh, 15001, Reason::UnresolvedCommand),
        (ssh_args, &ssh, 15001, Reason::UnresolvedCommand),
    ] {
        let context = serde_json::from_value(context.clone()).unwrap();
        let argv: Vec<_> = argv.iter().map(std::ffi::OsString::from).collect();
        let classification = super::super::metadata::classify_with_context(&argv, &context, || now);
        assert_eq!(classification.reason, reason, "{argv:?}");
        let proven = matches!(reason, Reason::GhWorkflowRun | Reason::SshRemoteCommand);
        assert_eq!(
            (classification.locality, classification.uncertainty),
            if proven {
                (Locality::Remote, Uncertainty::None)
            } else {
                (Locality::Unknown, Uncertainty::Observe)
            },
            "{argv:?}"
        );

        let root = std::env::temp_dir().join(format!(
            "arc-legacy-route-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let socket = root.join("s");
        let listener = UnixListener::bind(&socket).unwrap();
        let server = thread::spawn(move || {
            let next = || {
                let mut fd = libc::pollfd {
                    fd: listener.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                assert_eq!(
                    unsafe { libc::poll(&mut fd, 1, 60_000) },
                    1,
                    "legacy registration watchdog"
                );
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(60)))
                    .unwrap();
                let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
                assert_eq!(request.protocol_version, 1);
                (stream, request)
            };
            let (mut stream, first) = next();
            let (mut stream, legacy) = if classification.is_endpoint_locality() {
                assert_eq!(first.operation, OPERATION_JOB_START_LOCALITY);
                assert!(arc_ipc::jobs::decode_start_locality(&first.payload).is_ok());
                arc_ipc::write_response(
                    &mut stream,
                    &arc_ipc::Response::error(
                        &first.request_id,
                        arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                    ),
                )
                .unwrap();
                let (stream, legacy) = next();
                assert_eq!(legacy.payload["job_id"], first.payload["job_id"]);
                (stream, legacy)
            } else {
                (stream, first)
            };
            assert_eq!(legacy.operation, OPERATION_JOB_START);
            let decoded = arc_ipc::jobs::decode_start(&legacy.payload).unwrap();
            let StartRequest::Register {
                classification: wire,
                ..
            } = decoded
            else {
                panic!("expected register")
            };
            assert_eq!(wire, classification.legacy());
            if !proven {
                assert_eq!(
                    (wire.classification, wire.locality, wire.uncertainty),
                    (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe)
                );
            }
            arc_ipc::write_response(
                &mut stream,
                &arc_ipc::Response::ok(&legacy.request_id, snapshot(&legacy)),
            )
            .unwrap();
        });
        let input = StartRequest::Register {
            job_id: JobId::try_from("e".repeat(32)).unwrap(),
            observer: ProcessIdentity::capture(std::process::id()).unwrap(),
            classification,
            usage: None,
        };
        let result = start_until(
            &socket,
            input,
            Instant::now() + Duration::from_secs(10),
            &|| false,
        )
        .unwrap();
        assert_eq!(result.job.classification, classification.legacy());
        server.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
