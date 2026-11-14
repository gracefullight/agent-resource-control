use crate::{fixture, support::*};
use arc_ipc::{
    governor::*,
    traffic::{self, JobRequest, JobResult, Resources},
    Response,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::{fs::symlink, net::UnixListener, process::ExitStatusExt},
    process::Command,
    thread,
};

#[test]
fn accepted_fixture_streams_have_bounded_bidirectional_io() {
    use std::{os::fd::AsRawFd, os::unix::net::UnixStream, time::Duration};

    let socket = Socket::new();
    let mut client = UnixStream::connect(socket.root.join("s")).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    client
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let request = arc_ipc::Request::new("fixture-io", "fixture.roundtrip");
    arc_ipc::write_message(&mut client, &request).unwrap();
    let (mut accepted, observed) = next(&socket.listener);
    assert_eq!(observed, request);
    // Accept must normalize macOS's inherited O_NONBLOCK before any frame read.
    let flags = unsafe { libc::fcntl(accepted.as_raw_fd(), libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(flags & libc::O_NONBLOCK, 0);
    assert_eq!(
        accepted.read_timeout().unwrap(),
        Some(Duration::from_secs(5))
    );
    assert_eq!(
        accepted.write_timeout().unwrap(),
        Some(Duration::from_secs(5))
    );
    reply(&mut accepted, &observed, json!({"roundtrip": true}));
    let response: Response = arc_ipc::read_message(&mut client).unwrap();
    assert_eq!(
        response,
        Response::ok(&request.request_id, json!({"roundtrip": true}))
    );
}

pub(super) fn setup(socket: &Socket, body: &str) {
    fs::write(
        socket.root.join("package.json"),
        json!({"scripts":{"surprise":"bun run --cwd nested actual"}}).to_string(),
    )
    .unwrap();
    fs::create_dir(socket.root.join("nested")).unwrap();
    fs::write(
        socket.root.join("nested/package.json"),
        json!({"scripts":{"actual":body}}).to_string(),
    )
    .unwrap();
    symlink(
        env!("CARGO_BIN_EXE_arc-exec-fixture"),
        socket.root.join("bun"),
    )
    .unwrap();
}
pub(super) fn command(socket: &Socket) -> Command {
    let mut command = socket.cli();
    command
        .args(["exec", "--", "bun", "run", "surprise"])
        .env(
            "PATH",
            format!(
                "{}:{}",
                socket.root.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("ARC_FIXTURE_MODE", "exit17")
        .env("ARC_FIXTURE_MARKER", socket.root.join("starts"))
        .env_remove("ARC_TRAFFIC_LINEAGE");
    command
}
pub(super) fn unsupported(stream: &mut std::os::unix::net::UnixStream, request: &arc_ipc::Request) {
    arc_ipc::write_response(
        stream,
        &Response::error(&request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY),
    )
    .unwrap();
}

#[test]
fn negotiated_four_classes_preserve_original_argv_cwd_env_streams_exit_and_signal_once() {
    for (body, class, mode, queued, unreserved) in [
        ("vite build", "BUILD", "echo", true, false),
        ("bun test", "TEST", "exit17", false, false),
        ("biome check .", "LINT", "term", false, false),
        (
            "tsc --noEmit true --strict false -p a.json && tsc --noEmit --project b.json",
            "TYPECHECK",
            "exit17",
            false,
            true,
        ),
    ] {
        let socket = Socket::new();
        setup(&socket, body);
        let listener = socket.listener.try_clone().unwrap();
        let control = UnixListener::bind(socket.root.join("target-control")).unwrap();
        let root = socket.root.clone();
        let server = thread::spawn(move || {
            let (mut stream, request) = next(&listener);
            assert_eq!(
                request.operation,
                traffic::OPERATION_BUN_SCRIPTS_CAPABILITIES
            );
            assert!(request.payload.is_null());
            reply(
                &mut stream,
                &request,
                json!(traffic::Capabilities::scripts_supported()),
            );
            let (mut stream, mut request) = next(&listener);
            let GovernorJobRequest::Traffic {
                request:
                    JobRequest::Enqueue {
                        job_id,
                        observer,
                        classification,
                        lineage_context: Some(context),
                    },
            } = traffic::scripts::decode_job(&request.operation, &request.payload).unwrap()
            else {
                panic!("script enqueue")
            };
            assert_eq!(json!(classification.classification), class);
            assert_eq!(json!(classification.reason), format!("BUN_SCRIPT_{class}"));
            assert_eq!(
                request.payload.to_string().matches("surprise").count(),
                0,
                "No script/argv data over IPC"
            );
            if class == "TEST" || class == "LINT" {
                let original = request.clone();
                if class == "LINT" {
                    arc_ipc::write_response(
                        &mut stream,
                        &Response::error(&request.request_id, arc_ipc::ERROR_DB_BUSY),
                    )
                    .unwrap();
                }
                // TEST deliberately loses the first enqueue ACK. Admission
                // retries metadata; neither case may skip the launch fence.
                drop(stream);
                (stream, request) = next(&listener);
                assert_eq!(request.payload, original.payload);
                if class == "TEST" {
                    assert_eq!(
                        request, original,
                        "Lost-ACK replay preserves request identity too"
                    );
                }
                assert!(!root.join("starts").exists());
            }
            let reserved = json!(GovernorJobResult::Traffic {
                result: JobResult::Granted {
                    sequence: 1,
                    queued_at_ms: 100,
                    granted_at_ms: 101,
                    permit_generation: 1,
                    reservation: Resources {
                        cpu_millis: 1000,
                        memory_bytes: 1
                    },
                    lineage_context: context
                }
            });
            let eligible =
                json!({"state":"UNRESERVED","job_id":job_id,"switch_revision":5,"ready_at_ms":101});
            if queued {
                reply(
                    &mut stream,
                    &request,
                    json!(GovernorJobResult::Traffic {
                        result: JobResult::Queued {
                            sequence: 1,
                            queued_at_ms: 100,
                            reason: traffic::WaitReason::ClassWindow
                        }
                    }),
                );
                assert!(!root.join("starts").exists());
                let (mut stream, request) = next(&listener);
                let GovernorJobRequest::Traffic {
                    request:
                        JobRequest::Poll {
                            job_id: id,
                            observer: owner,
                        },
                } = traffic::scripts::decode_job(&request.operation, &request.payload).unwrap()
                else {
                    panic!("same identity poll")
                };
                assert_eq!(id, job_id);
                assert_eq!(owner, observer);
                reply(&mut stream, &request, reserved);
            } else {
                reply(
                    &mut stream,
                    &request,
                    if unreserved { eligible } else { reserved },
                );
            }
            let (mut stream, request) = next(&listener);
            assert_eq!(
                request.operation, OPERATION_LAUNCH,
                "class={class} payload={}",
                request.payload
            );
            let launch = decode_launch(&request.payload).unwrap();
            let release = if unreserved {
                GovernorLaunchResult::Unreserved {
                    switch_revision: 5,
                    release_id: "c".repeat(32),
                    valid_for_ms: 2000,
                }
            } else {
                GovernorLaunchResult::Reserved {
                    result: Box::new(arc_ipc::policy::launch::LaunchResult::Ready {
                        permit_generation: 1,
                        policy: fixture::fallback(),
                    }),
                }
            };
            reply(&mut stream, &request, json!(release));
            let ack = read(&mut stream);
            assert_eq!(ack.operation, OPERATION_LAUNCH_ACK);
            decode_launch_ack(&ack.payload, &launch, &release).unwrap();
            reply(&mut stream, &ack, json!({}));
            drop(stream);
            let (mut stream, request) = next(&listener);
            assert_eq!(
                request.operation,
                if unreserved {
                    OPERATION_JOB
                } else {
                    traffic::OPERATION_BUN_SCRIPTS_JOB
                }
            );
            let attached = if unreserved {
                decode_job(&request.payload).unwrap()
            } else {
                traffic::scripts::decode_job(&request.operation, &request.payload).unwrap()
            };
            let child = match attached {
                GovernorJobRequest::Attach {
                    job_id: id,
                    observer: owner,
                    child,
                    ..
                }
                | GovernorJobRequest::Traffic {
                    request:
                        JobRequest::Attach {
                            job_id: id,
                            observer: owner,
                            child,
                            ..
                        },
                } => {
                    assert_eq!(id, job_id);
                    assert_eq!(owner, observer);
                    child
                }
                _ => panic!("native attachment"),
            };
            reply(
                &mut stream,
                &request,
                json!({"state":"TRAFFIC","result":{"state":"RUNNING"}}),
            );
            let mut target = accept(&control);
            let hello: Value =
                serde_json::from_slice(&arc_ipc::read_frame(&mut target).unwrap()).unwrap();
            assert_eq!(hello["identity"], json!(child));
            target.write_all(b"x").unwrap();
            let (mut stream, request) = next(&listener);
            let completed = if unreserved {
                decode_job(&request.payload).unwrap()
            } else {
                traffic::scripts::decode_job(&request.operation, &request.payload).unwrap()
            };
            let outcome = match completed {
                GovernorJobRequest::Complete {
                    job_id: id,
                    observer: owner,
                    outcome,
                    ..
                }
                | GovernorJobRequest::Traffic {
                    request:
                        JobRequest::Complete {
                            job_id: id,
                            observer: owner,
                            outcome,
                            ..
                        },
                } => {
                    assert_eq!(id, job_id);
                    assert_eq!(owner, observer);
                    outcome
                }
                _ => panic!("terminal completion"),
            };
            if mode == "term" {
                assert!(
                    matches!(outcome, arc_ipc::jobs::Outcome::Signaled { signal, .. } if i32::from(signal) == libc::SIGTERM)
                );
            } else {
                assert_eq!(outcome, arc_ipc::jobs::Outcome::Exited { code: 17 });
            }
            reply(
                &mut stream,
                &request,
                json!({"state":"TRAFFIC","result":{"state":"COMPLETED","outcome":outcome,"queue_wait_ms":1}}),
            );
            let (mut stream, request) = next(&listener);
            assert_eq!(request.operation, arc_ipc::meter::OPERATION_METER_REPORT);
            assert_eq!(request.payload["job_id"], json!(job_id));
            assert!(request.payload["cpu_core_seconds"]["total"]
                .as_f64()
                .is_some());
            unsupported(&mut stream, &request);
        });
        let mut command = command(&socket);
        // All are opaque native arguments. They are valid Vite positional
        // entries; the fixed test/typecheck/lint cases use no forwarded args.
        if class == "BUILD" {
            command.args(["two words", "a'b", "$HOME", ";touch literal"]);
        }
        command
            .env("ARC_FIXTURE_MODE", mode)
            .env("PRIVATE_ENV", "preserved-value")
            .env("ARC_FIXTURE_CONTROL", socket.root.join("target-control"));
        let result = if mode == "echo" {
            command.stdin(std::process::Stdio::piped());
            let mut child = command.spawn().unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(b"native stdin\n")
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
            while child.try_wait().unwrap().is_none() {
                if std::time::Instant::now() >= deadline {
                    child.kill().unwrap();
                    panic!(
                        "native CLI timeout: {:?}",
                        child.wait_with_output().unwrap()
                    );
                }
                thread::sleep(std::time::Duration::from_millis(2));
            }
            child.wait_with_output().unwrap()
        } else {
            output(command)
        };
        server.join().unwrap_or_else(|_| {
            panic!(
                "class={class} status={} stderr={}",
                result.status,
                String::from_utf8_lossy(&result.stderr)
            )
        });
        if mode == "term" {
            assert_eq!(result.status.signal(), Some(libc::SIGTERM));
        } else {
            assert_eq!(
                result.status.code(),
                Some(17),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let text = String::from_utf8(result.stdout).unwrap();
            let data: Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
            let expected_args: Vec<_> = if class == "BUILD" {
                vec![
                    "run",
                    "surprise",
                    "two words",
                    "a'b",
                    "$HOME",
                    ";touch literal",
                ]
            } else {
                vec!["run", "surprise"]
            };
            assert_eq!(
                data["args"],
                json!(expected_args
                    .iter()
                    .map(|s| s.as_bytes().to_vec())
                    .collect::<Vec<_>>())
            );
            assert_eq!(data["environment"], json!(b"preserved-value".to_vec()));
            use std::os::unix::ffi::OsStrExt;
            assert_eq!(
                data["cwd"],
                json!(socket.root.canonicalize().unwrap().as_os_str().as_bytes())
            );
            assert!(String::from_utf8_lossy(&result.stderr).contains("fixture-stderr"));
            if mode == "echo" {
                assert!(text.starts_with("native stdin\n"));
            }
            if queued {
                assert!(String::from_utf8_lossy(&result.stderr).contains("QUEUED"));
            }
        }
        assert_eq!(
            fs::read_to_string(socket.root.join("starts"))
                .unwrap()
                .lines()
                .count(),
            1
        );
        assert!(!socket.root.join("literal").exists());
    }
}
