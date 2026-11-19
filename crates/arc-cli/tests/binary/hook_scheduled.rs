//! Focused, authenticated protocol peers exercise the actual scheduled spawn.
use arc_ipc::{
    governor::{self, GovernorJobRequest, GovernorJobResult, GovernorLaunchResult},
    traffic::{self, JobRequest, JobResult, Resources},
    Request, Response,
};
use serde_json::{json, Value};
use std::{
    ffi::{OsStr, OsString},
    fs,
    io::Write,
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{symlink, DirBuilderExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

const CAPTURE_HELLO: &[u8] = b"arc-hook-fixture-capture-v1";

#[path = "../../../arc-ipc/tests/fixtures/memory_pressure.rs"]
mod policy_fixture;

/// Capture to owned files so neither output stream can block the child.
pub(super) struct FixtureChild {
    child: super::hooks::fixture::OwnedChild,
    stdout: PathBuf,
    stderr: PathBuf,
}
impl FixtureChild {
    pub(super) fn spawn(mut command: Command, root: &Path) -> Self {
        Self::start(&mut command, root, false)
    }
    pub(super) fn spawn_nested(mut command: Command, root: &Path) -> Self {
        Self::start(&mut command, root, true)
    }
    fn start(command: &mut Command, root: &Path, nested: bool) -> Self {
        let id = arc_core::process_identity::random_job_id().unwrap();
        let stdout = root.join(format!("{id}.stdout"));
        let stderr = root.join(format!("{id}.stderr"));
        command
            .stdout(Stdio::from(fs::File::create(&stdout).unwrap()))
            .stderr(Stdio::from(fs::File::create(&stderr).unwrap()));
        let child = if nested {
            super::hooks::fixture::OwnedChild::spawn_nested(command, "hook semantics child")
        } else {
            super::hooks::fixture::OwnedChild::spawn(command, "hook semantics child")
        };
        Self {
            child,
            stdout,
            stderr,
        }
    }
    pub(super) fn write_input(&mut self, input: &[u8]) {
        let mut stdin = self.child.child_mut().stdin.take().unwrap();
        let input = input.to_vec();
        let (send, receive) = mpsc::channel();
        thread::spawn(move || {
            let _ = send.send(stdin.write_all(&input));
        });
        receive
            .recv_timeout(Duration::from_secs(120))
            .expect("hook semantics child did not consume stdin")
            .expect("write hook semantics child stdin");
    }
    pub(super) fn output(&mut self) -> Output {
        drop(self.child.child_mut().stdin.take());
        let status = self.child.wait();
        Output {
            status,
            stdout: fs::read(&self.stdout).unwrap(),
            stderr: fs::read(&self.stderr).unwrap(),
        }
    }
}
impl Drop for FixtureChild {
    fn drop(&mut self) {
        self.child.terminate_gracefully();
    }
}

struct Fixture {
    root: PathBuf,
    listener: UnixListener,
}
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from("/tmp").join(format!(
            "arc-hs-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        for tool in ["cargo", "bun"] {
            symlink(env!("CARGO_BIN_EXE_arc-exec-fixture"), root.join(tool)).unwrap();
        }
        fs::write(root.join("capture-key"), CAPTURE_HELLO).unwrap();
        fs::write(
            root.join("package.json"),
            br#"{"scripts":{"build":"vite build"}}"#,
        )
        .unwrap();
        let listener = UnixListener::bind(root.join("s")).unwrap();
        Self { root, listener }
    }
    fn shell(&self, command: &str, lineage: Option<&OsStr>) -> Command {
        self.shell_with("/bin/sh", command, lineage)
    }
    fn shell_with(&self, shell: &str, command: &str, lineage: Option<&OsStr>) -> Command {
        let mut result = Command::new(shell);
        result
            .env_clear()
            .args(["-c", command])
            .current_dir(&self.root)
            .env("PATH", &self.root)
            .env("HOME", &self.root)
            .env("ARC_SOCKET_PATH", self.root.join("s"))
            .env("ARC_FIXTURE_MODE", "hook-capture")
            .env("ARC_HOOK_CAPTURE_KEY", self.root.join("capture-key"))
            .env("ARC_HOOK_CAPTURE_MARKER", self.root.join("starts"))
            .env("ARC_TEST_OWNERSHIP", self.root.join("ownership"))
            .env("PRIVATE_ENV", OsString::from_vec(b"private-\xff".to_vec()))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(value) = lineage {
            result.env("ARC_TRAFFIC_LINEAGE", value);
        }
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            result.env("LLVM_PROFILE_FILE", profile);
        }
        result
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if !thread::panicking() {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
}

#[test]
fn capture_mode_without_fixture_handshake_does_not_activate() {
    let fixture = Fixture::new();
    let marker = fixture.root.join("ambient-marker");
    let mut command = Command::new(env!("CARGO_BIN_EXE_arc-exec-fixture"));
    command
        .env_clear()
        .env("ARC_FIXTURE_MODE", "hook-capture")
        .env("ARC_HOOK_CAPTURE_KEY", "/nonexistent/ambient-capture")
        .env("ARC_HOOK_CAPTURE_MARKER", &marker)
        .stdin(Stdio::null());
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let output = FixtureChild::spawn(command, &fixture.root).output();
    assert_eq!(output.status.code(), Some(17));
    assert_eq!(output.stderr, b"fixture-stderr\n");
    assert!(!marker.exists());
}

fn next(listener: &UnixListener) -> (UnixStream, Request) {
    use std::os::fd::AsRawFd;
    let mut descriptor = libc::pollfd {
        fd: listener.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    loop {
        let ready = unsafe { libc::poll(&mut descriptor, 1, 60_000) };
        if ready < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        assert!(ready > 0, "scheduled peer did not receive the next request");
        break;
    }
    let (mut stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let request = arc_ipc::read_message(&mut stream).unwrap();
    (stream, request)
}
fn reply(stream: &mut UnixStream, request: &Request, value: Value) {
    arc_ipc::write_response(stream, &Response::ok(&request.request_id, value)).unwrap();
}
fn unsupported(stream: &mut UnixStream, request: &Request) {
    arc_ipc::write_response(
        stream,
        &Response::error(&request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY),
    )
    .unwrap();
}

fn peer(listener: UnixListener, bun: bool, attached: mpsc::Sender<()>) -> (usize, usize, usize) {
    let (mut enqueues, mut attaches, mut completions) = (0, 0, 0);
    loop {
        let (mut stream, request) = next(&listener);
        let operation = request.operation.as_str();
        if operation == traffic::OPERATION_BUN_SCRIPTS_CAPABILITIES {
            assert!(bun);
            reply(
                &mut stream,
                &request,
                json!(traffic::Capabilities::scripts_supported()),
            );
        } else if operation == traffic::OPERATION_TRAFFIC_CAPABILITIES {
            assert!(!bun);
            reply(&mut stream, &request, json!({"capability":"traffic.v1"}));
        } else if operation == governor::OPERATION_LAUNCH {
            assert!(bun);
            let launch = governor::decode_launch(&request.payload).unwrap();
            let release = GovernorLaunchResult::Reserved {
                result: Box::new(arc_ipc::policy::launch::LaunchResult::Ready {
                    permit_generation: 1,
                    policy: policy_fixture::fallback(),
                }),
            };
            reply(&mut stream, &request, json!(release));
            let ack: Request = arc_ipc::read_message(&mut stream).unwrap();
            assert_eq!(ack.operation, governor::OPERATION_LAUNCH_ACK);
            governor::decode_launch_ack(&ack.payload, &launch, &release).unwrap();
            reply(&mut stream, &ack, json!({}));
        } else if operation == traffic::OPERATION_TRAFFIC_JOB
            || operation == traffic::OPERATION_BUN_SCRIPTS_JOB
        {
            let input = if bun {
                let GovernorJobRequest::Traffic { request: input } =
                    traffic::scripts::decode_job(operation, &request.payload).unwrap()
                else {
                    panic!("expected script Traffic request")
                };
                input
            } else {
                traffic::decode_job_for(operation, &request.payload).unwrap()
            };
            let result = match input {
                JobRequest::Enqueue {
                    classification,
                    lineage_context: Some(context),
                    ..
                } => {
                    enqueues += 1;
                    assert_eq!(json!(classification.classification), "BUILD");
                    assert_eq!(
                        json!(classification.reason),
                        if bun {
                            "BUN_SCRIPT_BUILD"
                        } else {
                            "CARGO_BUILD"
                        }
                    );
                    JobResult::Granted {
                        sequence: 1,
                        queued_at_ms: 1,
                        granted_at_ms: 1,
                        permit_generation: 1,
                        reservation: Resources {
                            cpu_millis: 1000,
                            memory_bytes: 1,
                        },
                        lineage_context: context,
                    }
                }
                JobRequest::Attach { .. } => {
                    attaches += 1;
                    JobResult::Running {}
                }
                JobRequest::Complete { outcome, .. } => {
                    completions += 1;
                    assert_eq!(outcome, arc_ipc::jobs::Outcome::Exited { code: 17 });
                    JobResult::Completed {
                        queue_wait_ms: 0,
                        outcome,
                    }
                }
                other => panic!("unexpected Traffic request: {other:?}"),
            };
            let is_attach = matches!(result, JobResult::Running {});
            let value = if bun {
                json!(GovernorJobResult::Traffic { result })
            } else {
                json!(result)
            };
            reply(&mut stream, &request, value);
            if is_attach {
                attached.send(()).unwrap();
            }
        } else if operation == arc_ipc::meter::OPERATION_METER_REPORT {
            unsupported(&mut stream, &request);
            break;
        } else {
            unsupported(&mut stream, &request);
        }
    }
    (enqueues, attaches, completions)
}

fn finish(command: Command, root: &Path, input: &[u8]) -> Output {
    let mut child = FixtureChild::spawn(command, root);
    child.write_input(input);
    child.output()
}

#[test]
fn fixture_captures_large_streams_and_reaps_on_unwind() {
    let fixture = Fixture::new();
    let mut command = Command::new("/bin/sh");
    command.args([
        "-c",
        "i=0; while [ \"$i\" -lt 8192 ]; do printf 0123456789abcdef; printf fedcba9876543210 >&2; i=$((i+1)); done",
    ]).stdin(Stdio::null());
    let output = FixtureChild::spawn(command, &fixture.root).output();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"0123456789abcdef".repeat(8192));
    assert_eq!(output.stderr, b"fedcba9876543210".repeat(8192));

    let mut pid = 0;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut command = Command::new("/bin/cat");
        command.stdin(Stdio::piped());
        let mut child = FixtureChild::spawn(command, &fixture.root);
        pid = child.child.child_mut().id() as i32;
        panic!("exercise fixture cleanup on unwind");
    }));
    assert!(result.is_err());
    let mut status = 0;
    assert_eq!(
        unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
}

#[path = "hook_child_tests.rs"]
mod child_tests;

#[test]
fn scheduled_hook_exec_preserves_actual_child_environment_streams_cwd_and_status() {
    for literal in ["cargo build", "bun run build"] {
        for lineage in [
            None,
            Some(OsStr::new("")),
            Some(OsStr::new(
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            )),
            Some(OsStr::from_bytes(b"supplied-\xff")),
        ] {
            for path in [None, Some(OsStr::new(""))] {
                let fixture = Fixture::new();
                let shell = |command: &str| {
                    let mut child = if path.is_some() {
                        fixture.shell_with("/bin/zsh", command, lineage)
                    } else {
                        fixture.shell(command, lineage)
                    };
                    if let Some(path) = path {
                        child.env("PATH", path);
                    }
                    child
                };
                let direct = finish(shell(literal), &fixture.root, b"stdin bytes\n");
                let executable = env!("CARGO_BIN_EXE_arcctl").replace('\'', "'\\''");
                let wrapped = format!("'{executable}' hook-exec -- {literal}");
                let listener = fixture.listener.try_clone().unwrap();
                let (send, receive) = mpsc::channel();
                let (result_send, result_receive) = mpsc::channel();
                thread::spawn(move || {
                    let _ = result_send.send(peer(listener, literal.starts_with("bun"), send));
                });
                let mut child = FixtureChild::spawn(shell(&wrapped), &fixture.root);
                receive
                    .recv_timeout(Duration::from_secs(60))
                    .expect("scheduled child was not attached");
                child.write_input(b"stdin bytes\n");
                let scheduled = child.output();
                assert_eq!(
                    result_receive
                        .recv_timeout(Duration::from_secs(120))
                        .expect("scheduled protocol peer did not finish"),
                    (1, 1, 1)
                );
                assert_eq!(direct.status.code(), Some(17));
                assert_eq!(scheduled.status.code(), direct.status.code());
                assert_eq!(
                    scheduled.stdout, direct.stdout,
                    "{literal}, lineage={lineage:?}, path={path:?}"
                );
                assert_eq!(
                    scheduled.stderr, direct.stderr,
                    "{literal}, lineage={lineage:?}, path={path:?}"
                );
                assert_eq!(
                    fs::read_to_string(fixture.root.join("starts"))
                        .unwrap()
                        .lines()
                        .count(),
                    2
                );
            }
        }
    }
}
