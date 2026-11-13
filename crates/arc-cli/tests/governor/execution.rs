use super::{fixture, support::*};
use arc_ipc::{
    governor::*,
    traffic::{JobRequest, JobResult, Resources},
    Response,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, Write},
    os::unix::{fs::symlink, net::UnixListener},
    thread,
};

#[derive(Clone, Copy, Debug)]
enum Case {
    New,
    Queued,
    Reserved,
    Resume,
    LostRelease,
    LostAck,
    Cancelled,
    LineageFailure,
    Held,
    InvalidAdmission,
}
#[test]
fn negotiated_new_queued_and_reserved_jobs_run_unreserved_once_with_native_lifecycle_and_meter() {
    for case in [
        Case::New,
        Case::Queued,
        Case::Reserved,
        Case::Resume,
        Case::LostAck,
        Case::LostRelease,
        Case::Cancelled,
        Case::InvalidAdmission,
    ] {
        execution(case);
    }
}
#[test]
fn lineage_failure_retracts_release_before_cancellation_needs_the_writer() {
    execution(Case::LineageFailure);
}

#[test]
fn a_pressure_hold_can_become_unreserved_without_reentering_the_reserved_checkpoint() {
    execution(Case::Held);
}

fn execution(case: Case) {
    let socket = Socket::new();
    let listener = socket.listener.try_clone().unwrap();
    let marker = socket.root.join("starts");
    let child_listener = UnixListener::bind(socket.root.join("target-control")).unwrap();
    symlink(
        env!("CARGO_BIN_EXE_arc-exec-fixture"),
        socket.root.join("cargo"),
    )
    .unwrap();
    let barrier = if matches!(case, Case::Held) {
        let listener = UnixListener::bind(socket.root.join("checkpoint")).unwrap();
        Some(thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut line = String::new();
            std::io::BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&line).unwrap()["mode"],
                "RESERVED"
            );
            stream.write_all(b"x").unwrap();
        }))
    } else {
        None
    };
    let server_root = socket.root.clone();
    let server = thread::spawn(move || {
        let (mut stream, request) = next(&listener);
        assert_eq!(request.operation, OPERATION_CAPABILITIES);
        reply(
            &mut stream,
            &request,
            json!(GovernorCapabilities::supported()),
        );
        let (mut stream, request) = next(&listener);
        assert_eq!(request.operation, OPERATION_JOB);
        let GovernorJobRequest::Traffic {
            request:
                JobRequest::Enqueue {
                    job_id,
                    observer,
                    lineage_context: Some(context),
                    ..
                },
        } = decode_job(&request.payload).unwrap()
        else {
            panic!("negotiated enqueue");
        };
        if matches!(case, Case::InvalidAdmission) {
            reply(
                &mut stream,
                &request,
                json!({"state":"TRAFFIC","result":{"state":"NOT_FOUND"}}),
            );
            drop(stream);
            cancelled_cleanup(&listener);
            return;
        }
        let eligible =
            json!({"state":"UNRESERVED","job_id":job_id,"switch_revision":5,"ready_at_ms":100});
        let reserved = json!(GovernorJobResult::Traffic {
            result: JobResult::Granted {
                sequence: 1,
                queued_at_ms: 100,
                granted_at_ms: 100,
                permit_generation: 1,
                reservation: Resources {
                    cpu_millis: 1000,
                    memory_bytes: 1
                },
                lineage_context: context.clone()
            }
        });
        if matches!(case, Case::Queued) {
            reply(
                &mut stream,
                &request,
                json!({"state":"TRAFFIC","result":{"state":"QUEUED","sequence":1,"queued_at_ms":100,"reason":"CLASS_WINDOW"}}),
            );
            let (mut stream, request) = next(&listener);
            assert_eq!(request.operation, OPERATION_JOB);
            assert!(matches!(
                decode_job(&request.payload).unwrap(),
                GovernorJobRequest::Traffic {
                    request: JobRequest::Poll { .. }
                }
            ));
            reply(&mut stream, &request, eligible.clone());
        } else {
            reply(
                &mut stream,
                &request,
                if matches!(case, Case::Reserved | Case::LineageFailure | Case::Held) {
                    reserved
                } else {
                    eligible.clone()
                },
            );
        }
        let (mut stream, request) = next(&listener);
        assert_eq!(request.operation, OPERATION_LAUNCH);
        let mut launch = decode_launch(&request.payload).unwrap();
        assert_eq!(
            matches!(launch, GovernorLaunchRequest::Reserved { .. }),
            matches!(case, Case::Reserved | Case::LineageFailure | Case::Held)
        );
        if matches!(case, Case::Held) {
            use arc_ipc::policy::*;
            let mut policy = fixture::fallback();
            policy.mode = PolicyMode::MemoryHold;
            policy.reason = PolicyReason::MemoryCritical;
            policy.observation = ObservationSummary {
                availability: ObservationAvailability::Available,
                pressure: Some(arc_core::meter::MemoryPressure::Critical),
                age_ms: Some(0),
                stale_after_ms: Some(3000),
                sampler: Some(SamplerIdentity {
                    daemon_instance: arc_ipc::jobs::ProcessIdentity::capture(std::process::id())
                        .unwrap(),
                    generation: 1,
                    monotonic_epoch: 1,
                }),
                sequence: Some(1),
                observed_at_ms: Some(100),
                observed_monotonic_ms: Some(100),
            };
            reply(
                &mut stream,
                &request,
                json!(GovernorLaunchResult::Reserved {
                    result: Box::new(arc_ipc::policy::launch::LaunchResult::Held { policy })
                }),
            );
            drop(stream);
            let (fresh_stream, fresh_request) = next(&listener);
            stream = fresh_stream;
            launch = decode_launch(&fresh_request.payload).unwrap();
            reply(
                &mut stream,
                &fresh_request,
                json!({"state":"UNRESERVED","switch_revision":5,"release_id":"cccccccccccccccccccccccccccccccc","valid_for_ms":2000}),
            );
        } else if matches!(case, Case::Resume) {
            reply(&mut stream, &request, json!({"state":"RETRY"}));
            drop(stream);
            let (mut poll_stream, poll_request) = next(&listener);
            assert_eq!(poll_request.operation, OPERATION_JOB);
            reply(
                &mut poll_stream,
                &poll_request,
                json!(GovernorJobResult::Traffic {
                    result: JobResult::Granted {
                        sequence: 1,
                        queued_at_ms: 100,
                        granted_at_ms: 101,
                        permit_generation: 2,
                        reservation: Resources {
                            cpu_millis: 1000,
                            memory_bytes: 1
                        },
                        lineage_context: context.clone()
                    }
                }),
            );
            let (fresh_stream, fresh_request) = next(&listener);
            stream = fresh_stream;
            launch = decode_launch(&fresh_request.payload).unwrap();
            // Resume-before-claim RETRY is polled rather than treated as a release.
            let release = json!(GovernorLaunchResult::Reserved {
                result: Box::new(arc_ipc::policy::launch::LaunchResult::Ready {
                    permit_generation: 2,
                    policy: fixture::fallback()
                })
            });
            reply(&mut stream, &fresh_request, release.clone());
        } else if matches!(case, Case::LostRelease) {
            drop(stream);
            cancelled_cleanup(&listener);
            return;
        } else if matches!(case, Case::Cancelled) {
            reply(&mut stream, &request, json!({"state":"CANCELLED"}));
            drop(stream);
            cancelled_cleanup(&listener);
            return;
        } else if matches!(case, Case::LineageFailure) {
            fs::create_dir(server_root.join(format!(
                ".traffic-lineage-{}-{}",
                observer.pid, observer.start_token
            )))
            .unwrap();
            reply(
                &mut stream,
                &request,
                json!(GovernorLaunchResult::Reserved {
                    result: Box::new(arc_ipc::policy::launch::LaunchResult::Ready {
                        permit_generation: 1,
                        policy: fixture::fallback()
                    })
                }),
            );
        } else {
            reply(
                &mut stream,
                &request,
                json!({"state":"UNRESERVED","switch_revision":5,"release_id":"cccccccccccccccccccccccccccccccc","valid_for_ms":2000}),
            );
        }
        let ack = read(&mut stream);
        assert_eq!(ack.operation, OPERATION_LAUNCH_ACK);
        let result = if matches!(case, Case::Resume | Case::LineageFailure) {
            GovernorLaunchResult::Reserved {
                result: Box::new(arc_ipc::policy::launch::LaunchResult::Ready {
                    permit_generation: if matches!(case, Case::LineageFailure) {
                        1
                    } else {
                        2
                    },
                    policy: fixture::fallback(),
                }),
            }
        } else {
            GovernorLaunchResult::Unreserved {
                switch_revision: 5,
                release_id: "c".repeat(32),
                valid_for_ms: 2000,
            }
        };
        assert!(decode_launch_ack(&ack.payload, &launch, &result).is_ok());
        if matches!(case, Case::LineageFailure) {
            assert_eq!(ack.payload["ack"]["state"], "UNSPAWNED");
            reply(&mut stream, &ack, json!({}));
            drop(stream);
            cancelled_cleanup(&listener);
            return;
        }
        if !matches!(case, Case::LostAck) {
            reply(&mut stream, &ack, json!({}));
        }
        drop(stream);
        let (mut stream, request) = next(&listener);
        assert_eq!(request.operation, OPERATION_JOB);
        let attached = decode_job(&request.payload).unwrap();
        let attached_child = match &attached {
            GovernorJobRequest::Attach { child, .. }
            | GovernorJobRequest::Traffic {
                request: JobRequest::Attach { child, .. },
            } => child.clone(),
            _ => panic!("native attach: case={case:?} request={}", request.payload),
        };
        if matches!(case, Case::Resume) {
            assert!(matches!(
                attached,
                GovernorJobRequest::Traffic {
                    request: JobRequest::Attach {
                        permit_generation: 2,
                        ..
                    }
                }
            ));
        } else {
            let GovernorJobRequest::Attach {
                job_id: attached,
                observer: owner,
                release_id,
                child,
            } = attached
            else {
                panic!(
                    "unreserved attach: case={case:?} request={}",
                    request.payload
                );
            };
            assert_eq!(attached, job_id);
            assert_eq!(owner, observer);
            assert_eq!(release_id, "c".repeat(32));
            assert!(child.is_valid());
            let witness =
                fs::read(server_root.join(format!(".traffic-witness-{}", job_id.as_str())))
                    .unwrap();
            assert_eq!(
                &witness[40..104],
                format!("{release_id}{release_id}").as_bytes(),
                "unreserved recovery authenticates the release, including reserved conversion"
            );
        }
        reply(
            &mut stream,
            &request,
            json!({"state":"TRAFFIC","result":{"state":"RUNNING"}}),
        );
        // Keep this actual native target alive through attachment. A fast exit
        // is separately valid, but cannot prove this fixture's live Attach and
        // native Meter path. Release from the observed child handshake only.
        let (mut target, _) = child_listener.accept().unwrap();
        target
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let hello: Value =
            serde_json::from_slice(&arc_ipc::read_frame(&mut target).unwrap()).unwrap();
        assert_eq!(hello["identity"], json!(attached_child));
        target.write_all(b"x").unwrap();
        let (mut stream, request) = next(&listener);
        let outcome = match decode_job(&request.payload).unwrap() {
            GovernorJobRequest::Complete { outcome, .. } if !matches!(case, Case::Resume) => {
                outcome
            }
            GovernorJobRequest::Traffic {
                request:
                    JobRequest::Complete {
                        outcome,
                        permit_generation: 2,
                        ..
                    },
            } if matches!(case, Case::Resume) => outcome,
            _ => panic!("mode-bound completion"),
        };
        reply(
            &mut stream,
            &request,
            json!({"state":"TRAFFIC","result":{"state":"COMPLETED","outcome":outcome,"queue_wait_ms":0}}),
        );
        let (mut stream, request) = next(&listener);
        assert_eq!(request.operation, arc_ipc::meter::OPERATION_METER_REPORT);
        assert!(request.payload["cpu_core_seconds"]["total"]
            .as_f64()
            .is_some());
        // Actual native measurements reached the real client reporter. This
        // peer deliberately makes no claim of persisted daemon measurements.
        arc_ipc::write_response(
            &mut stream,
            &Response::error(request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY),
        )
        .unwrap();
    });
    let mut command = socket.cli();
    command
        .args(["exec", "--"])
        .arg("cargo")
        .args(["test", "--exact", "two words"])
        .env(
            "PATH",
            format!(
                "{}:{}",
                socket.root.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("ARC_FIXTURE_MODE", "exit17")
        .env("ARC_FIXTURE_MARKER", &marker)
        .env("ARC_FIXTURE_CONTROL", socket.root.join("target-control"))
        .env("PRIVATE_ENV", "literal-value")
        .env_remove("ARC_TRAFFIC_LINEAGE");
    if matches!(case, Case::Held) {
        command
            .env("ARC_PRESSURE_ACCEPTANCE_STAGE", "reserved")
            .env(
                "ARC_PRESSURE_ACCEPTANCE_BARRIER",
                socket.root.join("checkpoint"),
            );
    }
    let result = output(command);
    if let Some(barrier) = barrier {
        barrier.join().unwrap();
    }
    server.join().unwrap();
    if matches!(
        case,
        Case::LostRelease | Case::Cancelled | Case::LineageFailure | Case::InvalidAdmission
    ) {
        assert!(!result.status.success());
        assert!(!marker.exists());
    } else {
        assert_eq!(
            result.status.code(),
            Some(17),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
        let metadata: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(
            metadata["args"],
            json!([b"test".to_vec(), b"--exact".to_vec(), b"two words".to_vec()])
        );
        assert!(String::from_utf8_lossy(&result.stderr).contains("fixture-stderr"));
    }
}
fn cancelled_cleanup(listener: &std::os::unix::net::UnixListener) {
    let (mut stream, request) = next(listener);
    assert_eq!(request.operation, OPERATION_JOB);
    assert!(matches!(
        decode_job(&request.payload).unwrap(),
        GovernorJobRequest::Traffic {
            request: JobRequest::Cancel { .. }
        }
    ));
    reply(
        &mut stream,
        &request,
        json!({"state":"TRAFFIC","result":{"state":"CANCELLED"}}),
    );
}
