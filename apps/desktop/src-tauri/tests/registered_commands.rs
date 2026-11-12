//! Real Tauri IPC dispatch with MockRuntime and authenticated same-user sockets.
//! Socket replies below are deterministic adversarial fixtures, not real-daemon
//! acceptance. The integration owner separately exercises DesktopBridge there.
#![cfg(unix)]

use arc_desktop::{
    bridge::DesktopBridge,
    configure_with_ports,
    error::{DesktopError, ErrorCode},
    lifecycle::{CreditLink, CreditOpener, DesktopPorts, DesktopShell},
};
use arc_ipc::{
    jobs::*, Classification, JobClass, Locality, Reason, Request, Response, Uncertainty,
};
use serde_json::{json, Value};
use std::{
    io::Write,
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{
    ipc::{CallbackFn, InvokeBody},
    test::{mock_builder, mock_context, noop_assets, MockRuntime},
    webview::InvokeRequest,
};

const JOB: &str = "0123456789abcdef0123456789abcdef";
const OTHER_JOB: &str = "abcdef0123456789abcdef0123456789";
const RECEIVED: u64 = 900;
const SENTINEL: &str = "PRIVATE_FIXTURE_ONLY_NEVER_DISPLAY";

#[derive(Default)]
struct Ports {
    links: Mutex<Vec<CreditLink>>,
    opener_fails: AtomicBool,
}
impl DesktopShell for Ports {
    fn hide_main(&self) -> Result<(), DesktopError> {
        Ok(())
    }
    fn show_main(&self) -> Result<(), DesktopError> {
        Ok(())
    }
    fn unminimize_main(&self) -> Result<(), DesktopError> {
        Ok(())
    }
    fn focus_main(&self) -> Result<(), DesktopError> {
        Ok(())
    }
    fn exit_desktop(&self) {}
}
impl CreditOpener for Ports {
    fn open(&self, link: CreditLink) -> Result<(), DesktopError> {
        self.links.lock().unwrap().push(link);
        if self.opener_fails.load(Ordering::Relaxed) {
            Err(DesktopError::new(ErrorCode::OpenerFailed))
        } else {
            Ok(())
        }
    }
}

struct App {
    _app: tauri::App<MockRuntime>,
    window: tauri::WebviewWindow<MockRuntime>,
    ports: Arc<Ports>,
}
impl App {
    fn new(bridge: DesktopBridge) -> Self {
        let ports = Arc::new(Ports::default());
        let app = configure_with_ports(
            mock_builder(),
            bridge,
            DesktopPorts {
                shell: ports.clone(),
                opener: ports.clone(),
            },
        )
        .build(mock_context(noop_assets()))
        .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        Self {
            _app: app,
            window,
            ports,
        }
    }
    fn invoke(&self, cmd: &str, body: Value) -> Result<Value, Value> {
        invoke(&self.window, cmd, body)
    }
}

fn invoke(
    window: &tauri::WebviewWindow<MockRuntime>,
    cmd: &str,
    body: Value,
) -> Result<Value, Value> {
    tauri::test::get_ipc_response(
        window,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    )
    .map(|body| body.deserialize().unwrap())
}

struct Peer {
    path: PathBuf,
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
    requests: Arc<AtomicUsize>,
}
impl Peer {
    fn new(count: usize, mut reply: impl FnMut(&mut UnixStream, Request) + Send + 'static) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "arc-d-{}-{}.sock",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(AtomicUsize::new(0));
        let stopped = stop.clone();
        let observed = requests.clone();
        let join = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            for _ in 0..count {
                let mut stream = loop {
                    if stopped.load(Ordering::Acquire) {
                        return;
                    }
                    assert!(Instant::now() < deadline, "fixture accept deadline");
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(1))
                        }
                        Err(_) => panic!("fixture accept failed"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let request = arc_ipc::read_message(&mut stream).unwrap();
                observed.fetch_add(1, Ordering::Relaxed);
                reply(&mut stream, request);
            }
        });
        Self {
            path,
            stop,
            join: Some(join),
            requests,
        }
    }
    fn result(result: Value) -> Self {
        Self::new(1, move |stream, request| {
            arc_ipc::write_response(stream, &Response::ok(request.request_id, result.clone()))
                .unwrap();
        })
    }
    fn bridge(&self) -> DesktopBridge {
        DesktopBridge::with_socket_and_clock(self.path.clone(), || RECEIVED)
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let result = self.join.take().unwrap().join();
        let _ = std::fs::remove_file(&self.path);
        if !thread::panicking() {
            result.unwrap();
        }
    }
}

fn health() -> Value {
    json!({ "daemon": "running", "protocol_version": 1, "sweeper": "off" })
}
fn error(code: &str, retryable: bool) -> Value {
    json!({"code": code, "retryable": retryable})
}
fn summary() -> Value {
    json!({"job_id": JOB,"phase":"COMPLETED","registered_at_ms":100,"updated_at_ms":150,"reason":"OBSERVED_EXIT"})
}
fn recent() -> Value {
    json!({"kind":"RECENT","jobs":[summary()],"truncated":false,"observed_at_ms":160,"liveness_fresh":false})
}

fn snapshot(code: u8) -> JobSnapshot {
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    JobSnapshot {
        job_id: JobId::try_from(JOB.to_owned()).unwrap(),
        phase: Phase::Completed,
        classification: Classification {
            classification: JobClass::Build,
            locality: Locality::HostLocal,
            reason: Reason::CargoBuild,
            uncertainty: Uncertainty::None,
        },
        observer: observer.clone(),
        observer_verified_at_ms: 100,
        child: Some(observer),
        child_verified_at_ms: Some(120),
        observation: IdentityObservation {
            target: ObservationTarget::Child,
            status: ObservationStatus::Missing,
            observed_at_ms: 150,
            reason: LifecycleReason::ObservedExit,
        },
        reason: LifecycleReason::ObservedExit,
        outcome: Some(Outcome::Exited { code }),
        registered_at_ms: 100,
        launch_claimed_at_ms: Some(110),
        completed_at_ms: Some(140),
        updated_at_ms: 150,
        revision: 3,
    }
}
fn found(job: JobSnapshot) -> Value {
    assert!(
        job.is_valid(),
        "test snapshot must obey production wire validation"
    );
    json!({"kind":"FOUND","job":job})
}
fn projected(code: u8) -> Value {
    json!({"kind":"found","job":{
        "job_id":JOB,"phase":"COMPLETED","reason":"OBSERVED_EXIT",
        "observation":{"target":"CHILD","status":"MISSING","observed_at_ms":150,"reason":"OBSERVED_EXIT"},
        "outcome":{"kind":"EXITED","code":code},"registered_at_ms":100,"launch_claimed_at_ms":110,
        "completed_at_ms":140,"updated_at_ms":150,"revision":3
    }})
}

// Include at crate scope to preserve the existing Cargo test identities.
include!("registered_commands_support/wire_projection.rs");
include!("registered_commands_support/input_validation.rs");
include!("registered_commands_support/protocol_validation.rs");
include!("registered_commands_support/status_validation.rs");
include!("registered_commands_support/local_lifecycle.rs");
include!("registered_commands_support/job_meter.rs");
