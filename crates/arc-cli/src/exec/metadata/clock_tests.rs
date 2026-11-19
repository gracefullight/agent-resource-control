use super::*;

#[test]
fn expired_or_future_context_preserves_only_revalidated_local_script_admission() {
    let fixture = Fixture::new();
    fixture.manifest(
        "",
        serde_json::json!({"scripts":{
            "local":"vite build",
            "remote":"gh workflow run build.yml",
            "mixed":"vite build && gh workflow run build.yml"
        }}),
    );
    let context = arc_detector::EndpointContext {
        captured_at_ms: 10_000,
        facts: vec![arc_detector::EndpointFact {
            kind: arc_detector::EndpointKind::CiHost,
            selector: "gh".into(),
            endpoints: vec!["https://github.com".into()],
        }],
        ..Default::default()
    };
    // Invalid before the first decision, during script analysis, and after
    // metadata revalidation. A backward clock is as invalid as expired data.
    for times in [
        [15_001, 15_001, 15_001],
        [9_999, 9_999, 9_999],
        [10_000, 15_001, 15_001],
        [10_000, 9_999, 10_000],
        [10_000, 10_000, 15_001],
    ] {
        for name in ["local", "remote", "mixed"] {
            let args = vec![
                OsString::from("bun"),
                OsString::from("run"),
                OsString::from("--cwd"),
                fixture.0.as_os_str().to_owned(),
                OsString::from(name),
            ];
            let mut clock = times.into_iter();
            let result = classify_with_context(&args, &context, || clock.next().unwrap());
            if name == "local" {
                assert_eq!(result.classification, JobClass::Build, "{times:?}");
                assert_eq!(result.locality, arc_detector::Locality::HostLocal);
                assert_eq!(result.reason, arc_detector::Reason::BunScriptBuild);
                assert!(result.is_bun_script());
            } else {
                assert_eq!(result.classification, JobClass::Unknown, "{name} {times:?}");
                assert_eq!(result.locality, arc_detector::Locality::Unknown);
                assert_eq!(result.uncertainty, arc_detector::Uncertainty::Observe);
            }
        }
    }
}

#[test]
fn stale_context_local_build_reaches_the_admission_route() {
    use std::{
        os::unix::{fs::PermissionsExt, net::UnixListener},
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };

    // Isolate the production signal handlers from other concurrent unit tests.
    const CHILD: &str = "ARC_TEST_STALE_CONTEXT_ADMISSION_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                concat!(
                    module_path!(),
                    "::stale_context_local_build_reaches_the_admission_route"
                ),
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(70);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "admission child exceeded 70s: stdout={} stderr={}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            thread::sleep(Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "admission child failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let fixture = Fixture::new();
    fs::set_permissions(&fixture.0, fs::Permissions::from_mode(0o700)).unwrap();
    fixture.manifest("", serde_json::json!({"scripts":{"local":"vite build"}}));
    let args = vec![
        OsString::from("bun"),
        OsString::from("run"),
        OsString::from("--cwd"),
        fixture.0.as_os_str().to_owned(),
        OsString::from("local"),
    ];
    let context = arc_detector::EndpointContext {
        captured_at_ms: 10_000,
        ..Default::default()
    };
    let classification = classify_with_context(&args, &context, || 15_001);
    let socket = fixture.0.join("s");
    let listener = UnixListener::bind(&socket).unwrap();
    let server = thread::spawn(move || {
        let next = || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(60)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(60)))
                .unwrap();
            let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
            (stream, request)
        };
        let (mut stream, request) = next();
        assert_eq!(
            request.operation,
            arc_ipc::traffic::OPERATION_BUN_SCRIPTS_CAPABILITIES
        );
        arc_ipc::write_response(
            &mut stream,
            &arc_ipc::Response::ok(
                &request.request_id,
                serde_json::json!(arc_ipc::traffic::Capabilities::scripts_supported()),
            ),
        )
        .unwrap();
        let (mut stream, request) = next();
        let arc_ipc::governor::GovernorJobRequest::Traffic {
            request:
                arc_ipc::traffic::JobRequest::Enqueue {
                    classification,
                    lineage_context: Some(lineage_context),
                    ..
                },
        } = arc_ipc::traffic::scripts::decode_job(&request.operation, &request.payload).unwrap()
        else {
            panic!("expected a Bun build enqueue");
        };
        assert_eq!(classification.classification, JobClass::Build);
        assert_eq!(classification.locality, arc_detector::Locality::HostLocal);
        let response = arc_ipc::governor::GovernorJobResult::Traffic {
            result: arc_ipc::traffic::JobResult::Granted {
                sequence: 1,
                queued_at_ms: 100,
                granted_at_ms: 101,
                permit_generation: 1,
                reservation: arc_ipc::traffic::Resources {
                    cpu_millis: 1000,
                    memory_bytes: 1,
                },
                lineage_context,
            },
        };
        arc_ipc::write_response(
            &mut stream,
            &arc_ipc::Response::ok(&request.request_id, serde_json::json!(response)),
        )
        .unwrap();
    });
    let signals = crate::exec::signals::Signals::install(false).unwrap();
    let id = arc_ipc::jobs::JobId::try_from(arc_core::process_identity::random_job_id().unwrap())
        .unwrap();
    let observer = arc_ipc::jobs::ProcessIdentity::capture(std::process::id()).unwrap();
    let admitted = crate::exec::governor::prepare(
        &socket,
        &id,
        &observer,
        classification,
        Instant::now() + Duration::from_secs(60),
        &signals,
    );
    server.join().unwrap();
    assert!(matches!(
        admitted,
        Ok(crate::exec::traffic::Mode::Scheduled { .. })
    ));
}
