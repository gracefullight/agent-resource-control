//! Headless process fixture running the registered production desktop router.
use super::{now_ms, write_json, RealDaemon};
use crate::{execution, native};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use arc_desktop::{
    bridge::DesktopBridge,
    error::{DesktopError, ErrorCode},
    lifecycle::{CreditLink, CreditOpener, DesktopPorts, DesktopShell},
};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};
use tauri::{
    ipc::{CallbackFn, InvokeBody},
    test::{mock_builder, mock_context, noop_assets, MockRuntime},
    webview::InvokeRequest,
};

const CLIENT_RECEIPT: &str = "ARC_DESKTOP_TEST_CLIENT_RECEIPT";

fn invoke(window: &tauri::WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Value {
    tauri::test::get_ipc_response(
        window,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    )
    .map(|body| body.deserialize::<Value>().unwrap())
    .unwrap()
}

struct ClientExit {
    bridge: DesktopBridge,
    receipt: PathBuf,
    observations: Arc<Mutex<Value>>,
}
impl DesktopShell for ClientExit {
    fn hide_main(&self) -> Result<(), DesktopError> {
        panic!("client probe only quits")
    }
    fn show_main(&self) -> Result<(), DesktopError> {
        panic!("client probe only quits")
    }
    fn unminimize_main(&self) -> Result<(), DesktopError> {
        panic!("client probe only quits")
    }
    fn focus_main(&self) -> Result<(), DesktopError> {
        panic!("client probe only quits")
    }
    fn exit_desktop(&self) {
        assert!(
            self.bridge.is_shutdown(),
            "production quit must cancel reads before exit"
        );
        assert_eq!(
            self.bridge.daemon_health().unwrap_err().code,
            ErrorCode::Cancelled
        );
        let mut observations = self.observations.lock().unwrap();
        observations["clientIdentity"] =
            json!(ProcessIdentity::capture(std::process::id()).unwrap());
        observations["atMs"] = json!(now_ms());
        observations["quitPath"] = json!(
            "registered quit_desktop -> DesktopState::dispatch -> bridge.shutdown -> exit_desktop"
        );
        observations["bridgeCancelledBeforeExit"] = json!(true);
        write_json(&self.receipt, &observations);
        // This headless OS exit adapter terminates the real client process.
        // No native event loop is needed, and LLVM flushes its child profile.
        std::process::exit(0);
    }
}
impl CreditOpener for ClientExit {
    fn open(&self, _: CreditLink) -> Result<(), DesktopError> {
        panic!("client must not open credits")
    }
}

pub fn is_client_process() -> bool {
    std::env::var_os(CLIENT_RECEIPT).is_some()
}

pub fn run_client_process() -> ! {
    let receipt = PathBuf::from(std::env::var_os(CLIENT_RECEIPT).unwrap());
    let id = std::env::var("ARC_DESKTOP_TEST_JOB_ID").unwrap();
    let bridge = DesktopBridge::new();
    let observations = Arc::new(Mutex::new(json!({})));
    let port = Arc::new(ClientExit {
        bridge: bridge.clone(),
        receipt,
        observations: observations.clone(),
    });
    let app = arc_desktop::configure_with_ports(
        mock_builder(),
        bridge,
        DesktopPorts {
            shell: port.clone(),
            opener: port,
        },
    )
    .build(mock_context(noop_assets()))
    .unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let health = invoke(&window, "daemon_health", json!({}));
    assert_eq!(health["data"]["protocol_version"], 1);
    let recent = invoke(&window, "recent_jobs", json!({}));
    assert_eq!(recent["data"]["jobs"][0]["job_id"], id);
    let selected = invoke(&window, "job_by_id", json!({"job_id":id}));
    assert_eq!(selected["data"]["job"]["job_id"], id);
    *observations.lock().unwrap() =
        json!({"health": health, "recent": recent, "selected": selected});
    invoke(&window, "quit_desktop", json!({}));
    panic!("registered quit handler failed to exit the desktop client");
}

pub fn desktop_client(case: &RealDaemon, test: &str, id: &str) -> Value {
    let receipt = case.evidence.join("desktop-client.json");
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(CLIENT_RECEIPT, &receipt)
        .env("ARC_DESKTOP_TEST_JOB_ID", id)
        .env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket)
        .env(arc_core::ARC_DB_PATH, &case.fixture.database)
        .stdin(Stdio::null())
        .stdout(fs::File::create(case.evidence.join("desktop-client.stdout")).unwrap())
        .stderr(fs::File::create(case.evidence.join("desktop-client.stderr")).unwrap());
    case.profile_command(&mut command);
    let mut child = native::OwnedChild::new(command.spawn().unwrap());
    let identity = ProcessIdentity::capture(child.child.id()).unwrap();
    let status = execution::wait(&mut child.child);
    assert_eq!(status.code(), Some(0));
    let observed: Value = serde_json::from_slice(&fs::read(receipt).unwrap()).unwrap();
    assert_eq!(observed["clientIdentity"], json!(identity));
    assert_eq!(observed["bridgeCancelledBeforeExit"], true);
    assert_eq!(probe_live(&identity), IdentityStatus::Missing);
    json!({"identity": identity, "exitCode": status.code(), "receipt": observed,
        "profiles": case.profiles(&[identity.pid])})
}
