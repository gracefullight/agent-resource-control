//! Shared native callbacks and local shell ports. This module owns no daemon.
use crate::{
    bridge::DesktopBridge,
    error::{DesktopError, ErrorCode},
};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_opener::OpenerExt;

pub const MAIN_WINDOW: &str = "main";
pub const TRAY_ID: &str = "arc-desktop";
pub const TRAY_MENU: [(&str, &str); 2] = [
    ("open_dashboard", "Open Dashboard"),
    ("quit_desktop", "Quit Desktop (Daemon continues)"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreditLink {
    Github,
    Linkedin,
    Oma,
}

impl CreditLink {
    pub fn parse(value: &str) -> Result<Self, DesktopError> {
        match value {
            "github" => Ok(Self::Github),
            "linkedin" => Ok(Self::Linkedin),
            "oma" => Ok(Self::Oma),
            _ => Err(DesktopError::new(ErrorCode::InvalidRequest)),
        }
    }
    pub const fn url(self) -> &'static str {
        match self {
            Self::Github => "https://github.com/gracefullight",
            Self::Linkedin => "https://www.linkedin.com/in/gracefullight",
            Self::Oma => "https://github.com/first-fluke/oh-my-agent",
        }
    }
}

pub trait DesktopShell: Send + Sync {
    fn hide_main(&self) -> Result<(), DesktopError>;
    fn show_main(&self) -> Result<(), DesktopError>;
    fn unminimize_main(&self) -> Result<(), DesktopError>;
    fn focus_main(&self) -> Result<(), DesktopError>;
    fn exit_desktop(&self);
}

pub trait CreditOpener: Send + Sync {
    fn open(&self, link: CreditLink) -> Result<(), DesktopError>;
}

pub struct DesktopPorts {
    pub shell: Arc<dyn DesktopShell>,
    pub opener: Arc<dyn CreditOpener>,
}

pub struct DesktopState {
    pub bridge: DesktopBridge,
    ports: DesktopPorts,
    quitting: AtomicBool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleEvent {
    CloseRequested,
    OpenDashboard,
    Reopen,
    QuitDesktop,
}

impl DesktopState {
    pub fn new(bridge: DesktopBridge, ports: DesktopPorts) -> Self {
        Self {
            bridge,
            ports,
            quitting: AtomicBool::new(false),
        }
    }
    pub fn is_quitting(&self) -> bool {
        self.quitting.load(Ordering::Acquire)
    }

    pub fn dispatch(&self, event: LifecycleEvent) -> Result<(), DesktopError> {
        if event == LifecycleEvent::QuitDesktop {
            if !self.quitting.swap(true, Ordering::AcqRel) {
                self.bridge.shutdown();
                self.ports.shell.exit_desktop();
            }
            return Ok(());
        }
        if self.is_quitting() {
            return Err(DesktopError::new(ErrorCode::Cancelled));
        }
        match event {
            LifecycleEvent::CloseRequested => self.ports.shell.hide_main(),
            LifecycleEvent::OpenDashboard | LifecycleEvent::Reopen => {
                self.ports.shell.show_main()?;
                self.ports.shell.unminimize_main()?;
                self.ports.shell.focus_main()
            }
            LifecycleEvent::QuitDesktop => unreachable!(),
        }
    }

    pub fn open_credit_link(&self, link: CreditLink) -> Result<(), DesktopError> {
        self.ports.opener.open(link)
    }
}

/// CloseRequestApi has no public test constructor. The installed Tauri callback
/// supplies its real prevention method; deterministic tests inject that port.
pub fn close_requested(
    state: &DesktopState,
    label: &str,
    prevent_close: impl FnOnce(),
) -> Result<(), DesktopError> {
    if label == MAIN_WINDOW && !state.is_quitting() {
        prevent_close();
        state.dispatch(LifecycleEvent::CloseRequested)?;
    }
    Ok(())
}

pub fn on_window_event<R: Runtime>(window: &tauri::Window<R>, event: &tauri::WindowEvent) {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        let _ = close_requested(&window.state::<DesktopState>(), window.label(), || {
            api.prevent_close()
        });
    }
}

pub fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: tauri::menu::MenuEvent) {
    let state = app.state::<DesktopState>();
    let event = match event.id().as_ref() {
        "open_dashboard" => LifecycleEvent::OpenDashboard,
        "quit_desktop" => LifecycleEvent::QuitDesktop,
        _ => return,
    };
    let _ = state.dispatch(event);
}

pub fn on_tray_icon_event<R: Runtime>(app: &AppHandle<R>, event: tauri::tray::TrayIconEvent) {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
    if matches!(event, TrayIconEvent::Click { ref id, button: MouseButton::Left, button_state: MouseButtonState::Up, .. } if id.as_ref() == TRAY_ID)
    {
        let _ = app
            .state::<DesktopState>()
            .dispatch(LifecycleEvent::OpenDashboard);
    }
}

pub fn on_reopen<R: Runtime>(app: &AppHandle<R>) -> Result<(), DesktopError> {
    app.state::<DesktopState>().dispatch(LifecycleEvent::Reopen)
}

/// Close is prevented by the window callback before an exit can be requested.
/// An actual application exit (including macOS Quit) must remain available.
/// Cancel reads without requesting a second exit or touching the daemon.
pub fn exit_requested(state: &DesktopState) {
    state.quitting.store(true, Ordering::Release);
    state.bridge.shutdown();
}

/// The same callback is passed to App::run and exercised with MockRuntime.
pub fn on_run_event<R: Runtime>(app: &AppHandle<R>, event: tauri::RunEvent) {
    match event {
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => {
            let _ = on_reopen(app);
        }
        tauri::RunEvent::ExitRequested { .. } => exit_requested(&app.state::<DesktopState>()),
        tauri::RunEvent::Exit => app.state::<DesktopState>().bridge.shutdown(),
        _ => {}
    }
}

/// Native adapters remain compiled for production; tests can delegate window
/// calls to a real MockRuntime handle without creating a native event loop.
pub struct TauriShell<R: Runtime>(pub AppHandle<R>);
impl<R: Runtime> TauriShell<R> {
    fn main(&self) -> Result<tauri::WebviewWindow<R>, DesktopError> {
        self.0
            .get_webview_window(MAIN_WINDOW)
            .ok_or(DesktopError::new(ErrorCode::WindowUnavailable))
    }
}
impl<R: Runtime> DesktopShell for TauriShell<R> {
    fn hide_main(&self) -> Result<(), DesktopError> {
        self.main()?
            .hide()
            .map_err(|_| DesktopError::new(ErrorCode::WindowUnavailable))?;
        self.0
            .emit_to(MAIN_WINDOW, "desktop://visibility", false)
            .map_err(|_| DesktopError::new(ErrorCode::WindowUnavailable))
    }
    fn show_main(&self) -> Result<(), DesktopError> {
        self.main()?
            .show()
            .map_err(|_| DesktopError::new(ErrorCode::WindowUnavailable))
    }
    fn unminimize_main(&self) -> Result<(), DesktopError> {
        self.main()?
            .unminimize()
            .map_err(|_| DesktopError::new(ErrorCode::WindowUnavailable))
    }
    fn focus_main(&self) -> Result<(), DesktopError> {
        self.main()?
            .set_focus()
            .map_err(|_| DesktopError::new(ErrorCode::WindowUnavailable))?;
        self.0
            .emit_to(MAIN_WINDOW, "desktop://visibility", true)
            .map_err(|_| DesktopError::new(ErrorCode::WindowUnavailable))
    }
    fn exit_desktop(&self) {
        let _ = self.0.emit_to(MAIN_WINDOW, "desktop://quitting", ());
        self.0.exit(0);
    }
}

pub struct TauriCreditOpener<R: Runtime>(pub AppHandle<R>);
impl<R: Runtime> CreditOpener for TauriCreditOpener<R> {
    fn open(&self, link: CreditLink) -> Result<(), DesktopError> {
        self.0
            .opener()
            .open_url(link.url(), None::<&str>)
            .map_err(|_| DesktopError::new(ErrorCode::OpenerFailed))
    }
}

pub(crate) fn install_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    use tauri::{
        menu::{Menu, MenuItem},
        tray::TrayIconBuilder,
    };
    let open = MenuItem::with_id(app, TRAY_MENU[0].0, TRAY_MENU[0].1, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, TRAY_MENU[1].0, TRAY_MENU[1].1, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Agent Resource Control");
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
