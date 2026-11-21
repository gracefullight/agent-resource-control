//! Deterministic tests of the production callback adapters and real registered
//! commands. MockRuntime does not model OS visibility (is_visible is constant),
//! so a recording port observes requests while delegating window calls to it.
//! No native event loop, tray click or browser is started.
// Contract seam for the desktop shell lane: the private real daemon, built
// arcctl and held-job support of `daemon_bridge` are registered here so the
// close-to-hide test can compose them with this target's `App`.
#[cfg(unix)]
#[allow(dead_code, unused_imports)]
mod daemon_bridge_support;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/support/native_exec.rs"]
mod execution;
#[cfg(unix)]
mod lifecycle_support;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/support/native_fixture.rs"]
mod native;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/support/mod.rs"]
mod serialization;

use arc_desktop::{
    bridge::DesktopBridge,
    configure_with_ports,
    error::{DesktopError, ErrorCode},
    lifecycle::{
        self, CreditLink, CreditOpener, DesktopPorts, DesktopShell, DesktopState, TauriShell,
        TRAY_ID, TRAY_MENU,
    },
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use tauri::{
    ipc::{CallbackFn, InvokeBody},
    test::{mock_builder, mock_context, noop_assets, MockRuntime},
    webview::InvokeRequest,
    Manager,
};

#[derive(Default)]
struct Observed {
    calls: Vec<&'static str>,
    visible: bool,
    focused: bool,
    minimized: bool,
    exits: usize,
}

#[derive(Default)]
struct RecordingShell {
    native: Mutex<Option<TauriShell<MockRuntime>>>,
    observed: Mutex<Observed>,
}
impl RecordingShell {
    fn call(
        &self,
        name: &'static str,
        action: impl FnOnce(&TauriShell<MockRuntime>) -> Result<(), DesktopError>,
    ) -> Result<(), DesktopError> {
        self.observed.lock().unwrap().calls.push(name);
        action(self.native.lock().unwrap().as_ref().unwrap())
    }
}
impl DesktopShell for RecordingShell {
    fn hide_main(&self) -> Result<(), DesktopError> {
        self.call("hide", DesktopShell::hide_main)?;
        self.observed.lock().unwrap().visible = false;
        Ok(())
    }
    fn show_main(&self) -> Result<(), DesktopError> {
        self.call("show", DesktopShell::show_main)?;
        self.observed.lock().unwrap().visible = true;
        Ok(())
    }
    fn unminimize_main(&self) -> Result<(), DesktopError> {
        self.call("unminimize", DesktopShell::unminimize_main)?;
        self.observed.lock().unwrap().minimized = false;
        Ok(())
    }
    fn focus_main(&self) -> Result<(), DesktopError> {
        self.call("focus", DesktopShell::focus_main)?;
        self.observed.lock().unwrap().focused = true;
        Ok(())
    }
    fn exit_desktop(&self) {
        // AppHandle::exit is the sole native exit port, replaced to keep this
        // test process alive. There is deliberately no daemon/execution port.
        let mut observed = self.observed.lock().unwrap();
        observed.calls.push("exit_desktop");
        observed.exits += 1;
    }
}
impl CreditOpener for RecordingShell {
    fn open(&self, _: CreditLink) -> Result<(), DesktopError> {
        panic!("lifecycle must not open a URL")
    }
}

struct App {
    app: tauri::App<MockRuntime>,
    shell: Arc<RecordingShell>,
    bridge: DesktopBridge,
}
impl App {
    fn new(bridge: DesktopBridge) -> Self {
        let shell = Arc::new(RecordingShell::default());
        let app = configure_with_ports(
            mock_builder(),
            bridge.clone(),
            DesktopPorts {
                shell: shell.clone(),
                opener: shell.clone(),
            },
        )
        .build(mock_context(noop_assets()))
        .unwrap();
        *shell.native.lock().unwrap() = Some(TauriShell(app.handle().clone()));
        Self { app, shell, bridge }
    }
    fn window(&self) -> tauri::WebviewWindow<MockRuntime> {
        tauri::WebviewWindowBuilder::new(&self.app, "main", Default::default())
            .build()
            .unwrap()
    }
    fn state(&self) -> tauri::State<'_, DesktopState> {
        self.app.state()
    }
}

fn invoke(window: &tauri::WebviewWindow<MockRuntime>, cmd: &str) -> Result<Value, Value> {
    invoke_with(window, cmd, json!({}))
}

fn invoke_with(
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

fn menu(app: &tauri::AppHandle<MockRuntime>, id: &str) {
    lifecycle::on_menu_event(app, tauri::menu::MenuEvent { id: id.into() });
}

#[test]
fn close_request_hides_existing_window() {
    let app = App::new(DesktopBridge::new());
    let window = app.window();
    let prevented = AtomicUsize::new(0);
    // This is the prevention adapter installed by on_window_event. Tauri's
    // CloseRequestApi constructor is private; no fake enum-only reducer is used.
    lifecycle::close_requested(&app.state(), window.label(), || {
        prevented.fetch_add(1, Ordering::Relaxed);
    })
    .unwrap();
    assert_eq!(prevented.load(Ordering::Relaxed), 1);
    assert!(!app.shell.observed.lock().unwrap().visible);
    assert_eq!(app.shell.observed.lock().unwrap().calls, ["hide"]);
    assert_eq!(app.app.get_webview_window("main").unwrap(), window);
    assert_eq!(app.app.webview_windows().len(), 1);
    assert!(!app.bridge.is_shutdown());
    assert!(!app.state().is_quitting());
    assert_eq!(invoke(&window, "hide_dashboard"), Ok(Value::Null));
    assert_eq!(app.shell.observed.lock().unwrap().calls, ["hide", "hide"]);
    lifecycle::close_requested(&app.state(), "unrelated", || {
        panic!("wrong window prevented")
    })
    .unwrap();
    lifecycle::on_window_event(
        &window.as_ref().window(),
        &tauri::WindowEvent::Focused(false),
    );
    assert_eq!(app.shell.observed.lock().unwrap().calls.len(), 2);
}

// Contract seam: the app built by `configure_with_ports` with the production
// handlers reads a private real daemon through the registered bridge commands.
#[test]
#[cfg(unix)]
fn registered_app_reads_private_real_daemon() {
    let _serial = serialization::serial();
    let mut case = daemon_bridge_support::RealDaemon::isolated("lifecycle-seam");
    let daemon = case.identity();
    let app = App::new(case.bridge.clone());
    let window = app.window();
    let health = invoke(&window, "daemon_health").unwrap();
    assert_eq!(
        health["data"],
        json!({"daemon":"running","protocol_version":1,"sweeper":"off"})
    );
    let recent = invoke(&window, "recent_jobs").unwrap();
    assert_eq!(recent["data"]["jobs"], json!([]));
    assert_eq!(case.identity(), daemon);
    assert!(!app.bridge.is_shutdown());
    assert!(app.shell.observed.lock().unwrap().calls.is_empty());
    let root = case.fixture.root.clone();
    case.stop("lifecycle-seam");
    drop(case);
    assert!(
        !root.exists(),
        "a passing case removes its fixture directory"
    );
}

// DEC-003, DEC-004, ADR-002 and AT-GUI-01: the application registered by
// `configure_with_ports` hides its window on a close request while one private
// real daemon keeps its process identity and goes on serving the registered
// bridge commands and the built arcctl. The prevention port is injected as in
// `close_request_hides_existing_window`; no native window, tray, menu bar or
// screen is observed.
#[test]
#[cfg(unix)]
fn close_request_hides_main_and_same_daemon_serves_bridge_and_cli() {
    use arc_core::process_identity::{probe_live, IdentityStatus};
    use daemon_bridge_support::{detail, HeldJob, RealDaemon};
    let _serial = serialization::serial();
    let mut case = RealDaemon::isolated("close-to-hide");
    let daemon = case.identity();
    let app = App::new(case.bridge.clone());
    let window = app.window();
    assert_eq!(invoke(&window, "open_dashboard"), Ok(Value::Null));
    assert!(app.shell.observed.lock().unwrap().visible);

    let prevented = AtomicUsize::new(0);
    lifecycle::close_requested(&app.state(), window.label(), || {
        prevented.fetch_add(1, Ordering::Relaxed);
    })
    .unwrap();
    assert_eq!(prevented.load(Ordering::Relaxed), 1);
    let same_window_stays_hidden = || {
        let observed = app.shell.observed.lock().unwrap();
        assert_eq!(observed.calls, ["show", "unminimize", "focus", "hide"]);
        assert!(!observed.visible);
        assert_eq!(observed.exits, 0);
        assert_eq!(app.app.get_webview_window("main").unwrap(), window);
        assert_eq!(app.app.webview_windows().len(), 1);
        assert!(!app.state().is_quitting() && !app.bridge.is_shutdown());
    };
    same_window_stays_hidden();

    // After the close, the same daemon process answers the registered bridge
    // command and a request from the built CLI.
    assert_eq!(probe_live(&daemon), IdentityStatus::VerifiedMatch);
    assert_eq!(case.identity(), daemon);
    assert_eq!(
        invoke(&window, "daemon_health").unwrap()["data"],
        json!({"daemon":"running","protocol_version":1,"sweeper":"off"})
    );
    let status = lifecycle_support::arcctl(&case, &["status"]);
    assert!(status.status.success(), "{status:?}");
    assert_eq!(
        String::from_utf8(status.stdout).unwrap(),
        format!(
            "running. Socket: {}. Protocol: 1. Sweeper off.\n",
            case.fixture.socket.display()
        )
    );
    #[cfg(target_os = "macos")]
    {
        // Both replies state the answering daemon's own PID and start time.
        let bridge = lifecycle_support::through_bridge(|| invoke(&window, "meter_snapshot"));
        assert_eq!(bridge, daemon);
        assert_eq!(lifecycle_support::through_cli(&case), daemon);
    }

    // A job started through the CLI is read back through the bridge.
    let mut job = HeldJob::launch(&case, 17);
    let id = detail(&case.wait_running(&job.identity))
        .job_id
        .as_str()
        .to_owned();
    let read = || invoke_with(&window, "job_by_id", json!({"job_id": id})).unwrap()["data"].clone();
    let running = read();
    assert_eq!(running["kind"], "found");
    assert_eq!(running["job"]["job_id"], id);
    assert_eq!(running["job"]["phase"], "RUNNING");
    assert_eq!(running["job"]["outcome"], Value::Null);
    let recent = invoke(&window, "recent_jobs").unwrap();
    assert_eq!(recent["data"]["jobs"][0]["job_id"], id);
    assert_eq!(job.release_and_wait().code(), Some(17));
    let completed = read();
    assert_eq!(completed["job"]["job_id"], id);
    assert_eq!(completed["job"]["phase"], "COMPLETED");
    assert_eq!(
        completed["job"]["outcome"],
        json!({"kind":"EXITED","code":17})
    );
    let launches = std::fs::read_to_string(case.fixture.root.join("launches")).unwrap();
    assert_eq!(launches.lines().count(), 1);
    assert_eq!(case.identity(), daemon);
    same_window_stays_hidden();

    let root = case.fixture.root.clone();
    case.stop("close-to-hide");
    drop(case);
    assert!(
        !root.exists(),
        "a passing case removes its fixture directory"
    );
}

#[test]
fn reopen_reuses_window() {
    let app = App::new(DesktopBridge::new());
    let window = app.window();
    lifecycle::close_requested(&app.state(), "main", || {}).unwrap();
    menu(app.app.handle(), "open_dashboard");
    lifecycle::on_reopen(app.app.handle()).unwrap();
    assert_eq!(invoke(&window, "open_dashboard"), Ok(Value::Null));
    lifecycle::on_tray_icon_event(
        app.app.handle(),
        tauri::tray::TrayIconEvent::Click {
            id: TRAY_ID.into(),
            position: tauri::PhysicalPosition::default(),
            rect: tauri::Rect {
                position: tauri::PhysicalPosition::<i32>::default().into(),
                size: tauri::PhysicalSize::<u32>::new(16, 16).into(),
            },
            button: tauri::tray::MouseButton::Left,
            button_state: tauri::tray::MouseButtonState::Up,
        },
    );
    let observed = app.shell.observed.lock().unwrap();
    assert!(observed.visible && observed.focused && !observed.minimized);
    assert_eq!(
        observed.calls,
        [
            "hide",
            "show",
            "unminimize",
            "focus",
            "show",
            "unminimize",
            "focus",
            "show",
            "unminimize",
            "focus",
            "show",
            "unminimize",
            "focus"
        ]
    );
    drop(observed);
    assert_eq!(app.app.get_webview_window("main").unwrap(), window);
    assert_eq!(app.app.webview_windows().len(), 1);
    menu(app.app.handle(), "unknown");
    lifecycle::on_run_event(app.app.handle(), tauri::RunEvent::Ready);
    assert_eq!(
        TRAY_MENU,
        [
            ("open_dashboard", "Open Dashboard"),
            ("quit_desktop", "Quit Desktop (Daemon continues)")
        ]
    );
}

#[test]
#[cfg(unix)]
fn quit_desktop_never_stops_daemon() {
    use std::os::unix::net::UnixListener;
    // The real-daemon/active-child survival assertion belongs to daemon_bridge.
    // This idle socket detects any forbidden shutdown IPC or socket deletion.
    let path = std::env::temp_dir().join(format!("arc-lifecycle-{}.sock", std::process::id()));
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let app = App::new(DesktopBridge::from_socket_path(path.clone()));
    let window = app.window();
    menu(app.app.handle(), "quit_desktop");
    assert_eq!(invoke(&window, "quit_desktop"), Ok(Value::Null));
    assert!(app.state().is_quitting() && app.bridge.is_shutdown());
    assert_eq!(app.shell.observed.lock().unwrap().exits, 1);
    assert_eq!(app.shell.observed.lock().unwrap().calls, ["exit_desktop"]);
    assert!(path.exists(), "desktop must not unlink daemon socket");
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    lifecycle::exit_requested(&app.state());
    lifecycle::close_requested(&app.state(), "main", || {
        panic!("quitting close was prevented")
    })
    .unwrap();
    assert_eq!(
        invoke(&window, "open_dashboard"),
        Err(json!({"code":"cancelled","retryable":false}))
    );
    assert_eq!(
        app.bridge.daemon_health().unwrap_err().code,
        ErrorCode::Cancelled
    );
    lifecycle::on_run_event(app.app.handle(), tauri::RunEvent::Exit);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn registered_quit_command_reaches_the_same_desktop_exit_port() {
    let app = App::new(DesktopBridge::new());
    let window = app.window();
    assert_eq!(invoke(&window, "quit_desktop"), Ok(Value::Null));
    assert!(app.bridge.is_shutdown());
    assert_eq!(app.shell.observed.lock().unwrap().exits, 1);
    menu(app.app.handle(), "open_dashboard");
    assert_eq!(app.shell.observed.lock().unwrap().calls, ["exit_desktop"]);
}

#[test]
fn system_quit_is_allowed_and_cancels_only_desktop_reads() {
    let app = App::new(DesktopBridge::new());
    let window = app.window();
    assert!(!app.state().is_quitting());
    // The installed RunEvent::ExitRequested adapter deliberately never calls
    // prevent_exit. Close-to-hide is already handled by prevent_close.
    lifecycle::exit_requested(&app.state());
    assert!(app.state().is_quitting() && app.bridge.is_shutdown());
    assert_eq!(app.shell.observed.lock().unwrap().exits, 0);
    assert_eq!(invoke(&window, "quit_desktop"), Ok(Value::Null));
    assert_eq!(
        app.shell.observed.lock().unwrap().exits,
        0,
        "exit is already in progress"
    );
    lifecycle::close_requested(&app.state(), "main", || panic!("system quit intercepted")).unwrap();
}

#[test]
fn missing_window_errors_are_explicit_and_close_is_still_prevented() {
    let app = App::new(DesktopBridge::new());
    let prevented = AtomicUsize::new(0);
    assert_eq!(
        lifecycle::close_requested(&app.state(), "main", || {
            prevented.fetch_add(1, Ordering::Relaxed);
        }),
        Err(DesktopError::new(ErrorCode::WindowUnavailable))
    );
    assert_eq!(prevented.load(Ordering::Relaxed), 1);
    assert_eq!(
        lifecycle::on_reopen(app.app.handle()),
        Err(DesktopError::new(ErrorCode::WindowUnavailable))
    );
    let native = TauriShell(app.app.handle().clone());
    for result in [
        native.hide_main(),
        native.show_main(),
        native.unminimize_main(),
        native.focus_main(),
    ] {
        assert_eq!(result, Err(DesktopError::new(ErrorCode::WindowUnavailable)));
    }
}
