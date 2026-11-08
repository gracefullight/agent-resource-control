//! CLI-only fixtures use a local fake protocol peer; no shell or tools are run.
use arc_ipc::{Request, Response};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::{fs::DirBuilderExt, net::UnixListener},
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = arc_core::runtime_root().unwrap().join(format!(
            "c{:x}-{:x}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self { root }
    }
    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_arcctl"))
            .args(args)
            .env(arc_core::ARC_SOCKET_PATH, self.root.join("ipc"))
            .output()
            .unwrap()
    }
    fn peer(
        &self,
        args: &[&str],
        reply: impl FnOnce(Request, &mut std::os::unix::net::UnixStream) + Send,
    ) -> Output {
        let listener = UnixListener::bind(self.root.join("ipc")).unwrap();
        listener.set_nonblocking(true).unwrap();
        thread::scope(|scope| {
            let peer = scope.spawn(|| {
                let start = Instant::now();
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(start.elapsed() < Duration::from_secs(7), "CLI never sent a request");
                            thread::sleep(Duration::from_millis(1));
                        }
                        Err(_) => panic!("peer accept failed"),
                    }
                };
                // Accepted sockets may inherit the listener nonblocking flag on macOS.
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                let request: Request = arc_ipc::read_message(&mut stream).unwrap();
                assert_eq!(request.protocol_version, 1);
                assert_eq!(request.operation, arc_ipc::OPERATION_JOB_CLASSIFY);
                assert!(request.request_id.starts_with("arcctl-"));
                reply(request, &mut stream);
                assert!(matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock), "unexpected retry or diagnostic connection");
            });
            let output = self.cli(args);
            peer.join().unwrap();
            output
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn valid() -> Value {
    json!({"classification":"TEST","locality":"HOST_LOCAL","reason":"CARGO_TEST","uncertainty":"NONE"})
}
fn assert_failure(output: &Output, code: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with(code));
    assert!(!output.stderr.contains(&27));
}

#[test]
fn help_usage_privacy_and_absent_daemon() {
    let fixture = Fixture::new();
    let listener = UnixListener::bind(fixture.root.join("ipc")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let empty = fixture.cli(&[]);
    assert!(!empty.status.success());
    let text = String::from_utf8_lossy(&empty.stderr);
    for name in ["status", "doctor", "classify"] {
        assert!(text.contains(name));
    }
    for args in [&["--help"][..], &["classify", "--help"][..]] {
        let output = fixture.cli(args);
        assert!(output.status.success() && output.stderr.is_empty());
        assert!(!output.stdout.contains(&27));
        if args.len() == 2 {
            let text = String::from_utf8_lossy(&output.stdout);
            for expected in [
                "--json",
                "--shell",
                "one quoted argument",
                "before ARC receives input",
                "arcctl classify 'cargo test'",
                "arcctl classify --json 'gh workflow run ci.yml'",
                "cargo test && gh workflow run ci.yml",
                "cargo test --",
                "arcctl classify --shell bash -- 'cargo test'",
                "arcctl classify -- ''",
                "arcctl classify --shell powershell -- 'cargo test'",
            ] {
                assert!(text.contains(expected), "missing help term");
            }
        }
    }
    let secret = "synthetic-extra-argument-canary";
    for args in [
        vec!["classify"],
        vec!["classify", "cargo test", secret],
        vec!["classify", "--unknown", secret],
        vec!["classify", "--shell"],
    ] {
        let output = fixture.cli(&args);
        assert_failure(&output, "INVALID_REQUEST");
        assert!(!String::from_utf8_lossy(&output.stderr).contains(secret));
    }
    assert!(matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock));
    drop(listener);
    fs::remove_file(fixture.root.join("ipc")).unwrap();
    assert_failure(
        &fixture.cli(&["classify", "cargo test"]),
        "DAEMON_UNAVAILABLE",
    );
}

#[test]
fn literal_payloads_and_options_are_preserved_in_one_request() {
    for text in [
        "  cargo test  ",
        "cargo\ttest",
        "cargo test -- \"검증 λ\"",
        "cargo test 'gh run watch'",
        "cargo test $(never-run)",
    ] {
        for args in [
            vec!["classify", "--json", text],
            vec!["classify", text, "--json"],
            vec!["classify", "--shell", "zsh", "--json", "--", text],
            vec!["classify", text, "--shell", "bash", "--json"],
        ] {
            let fixture = Fixture::new();
            let output = fixture.peer(&args, |request, stream| {
                assert!(request.payload["command"] == text, "payload bytes changed");
                assert!(
                    request.payload["shell_kind"]
                        == if args.contains(&"zsh") {
                            "zsh"
                        } else if args.contains(&"bash") {
                            "bash"
                        } else {
                            "sh"
                        }
                );
                let response = Response::ok(request.request_id, valid());
                arc_ipc::write_response(stream, &response).unwrap();
            });
            assert!(output.status.success() && output.stderr.is_empty());
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                valid()
            );
        }
    }
}

#[test]
fn peer_failures_are_typed_correlated_and_never_reflected() {
    // Every case is one actual arcctl process and one local protocol exchange.
    for case in 0..16 {
        let fixture = Fixture::new();
        let output = fixture.peer(&["classify", "--json", "cargo test"], |request, stream| {
            let mut response = Response::ok(&request.request_id, valid());
            match case {
                0 => response.protocol_version = 2,
                1 => response = Response::error(&request.request_id, "PROTOCOL_INCOMPATIBLE"),
                2 => response = Response::error(&request.request_id, "UNSUPPORTED_CAPABILITY"),
                3 => response = Response::error(&request.request_id, "INVALID_REQUEST"),
                4 => response = Response::error(&request.request_id, "untrusted-secret-canary"),
                5 => response.request_id = "wrong-correlation".into(),
                6 => response.result = None,
                7 => response.result = Some(json!({"classification":"NEW_ENUM"})),
                8 => response.result = Some(json!({"classification":true,"locality":"HOST_LOCAL","reason":"CARGO_TEST","uncertainty":"NONE"})),
                9 => response.result.as_mut().unwrap()["locality"] = "REMOTE".into(),
                10 => response.result.as_mut().unwrap()["reason"] = "SSH_REMOTE_COMMAND".into(),
                11 => response.status = "unexpected".into(),
                12 => { // premature EOF
                    use std::io::Write;
                    stream.write_all(&100u32.to_be_bytes()).unwrap();
                    stream.write_all(b"{").unwrap();
                    return;
                }
                13 => { arc_ipc::write_frame(stream, b"{malformed-json").unwrap(); return; }
                14 => response.error = Some(arc_ipc::ErrorBody {code:"INVALID_REQUEST".into(),message:Some("untrusted-secret-canary".into())}),
                15 => response.result.as_mut().unwrap()["extra"] = "untrusted-secret-canary".into(),
                _ => unreachable!(),
            }
            if let Some(error) = response.error.as_mut() { error.message = Some("untrusted-secret-canary".into()); }
            arc_ipc::write_response(stream, &response).unwrap();
        });
        let expected = match case {
            0 | 1 => "PROTOCOL_INCOMPATIBLE",
            2 => "UNSUPPORTED_CAPABILITY",
            3 => "INVALID_REQUEST",
            _ => "IPC_ERROR",
        };
        assert_failure(&output, expected);
        assert!(!String::from_utf8_lossy(&output.stderr).contains("untrusted-secret-canary"));
        // A fresh process after an error can succeed without doctor/status or replay.
        fs::remove_file(fixture.root.join("ipc")).unwrap();
        let output = fixture.peer(&["classify", "--json", "cargo test"], |request, stream| {
            arc_ipc::write_response(stream, &Response::ok(request.request_id, valid())).unwrap();
        });
        assert!(output.status.success() && output.stderr.is_empty());
    }
}

#[test]
fn timeout_is_bounded_with_no_retry_or_partial_result() {
    let fixture = Fixture::new();
    let start = Instant::now();
    let output = fixture.peer(&["classify", "cargo test"], |_, _| {
        thread::sleep(Duration::from_millis(5500))
    });
    assert_failure(&output, "IPC_ERROR");
    assert!(start.elapsed() < Duration::from_secs(7));
}

#[test]
fn error_with_success_payload_is_rejected_before_error_mapping() {
    let fixture = Fixture::new();
    let output = fixture.peer(&["classify", "--json", "cargo test"], |request, stream| {
        let mut response = Response::error(request.request_id, "INVALID_REQUEST");
        response.result = Some(valid());
        arc_ipc::write_response(stream, &response).unwrap();
    });
    assert_failure(&output, "IPC_ERROR");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Pass exactly one"));
}
