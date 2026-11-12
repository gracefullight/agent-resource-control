//! Production registered router; only shell ports use MockRuntime. This is not
//! native visibility evidence; the separate Wry example observes real windows.
use arc_desktop::{
    bridge::DesktopBridge,
    error::DesktopError,
    lifecycle::{CreditLink, CreditOpener, DesktopPorts, DesktopShell},
};
use serde_json::Value;
use std::sync::Arc;
use tauri::{
    ipc::{CallbackFn, InvokeBody},
    test::{mock_builder, mock_context, noop_assets, MockRuntime},
    webview::InvokeRequest,
};

struct UnusedPorts;
impl DesktopShell for UnusedPorts {
    fn hide_main(&self) -> Result<(), DesktopError> {
        panic!("native read test does not hide windows")
    }
    fn show_main(&self) -> Result<(), DesktopError> {
        panic!("native read test does not show windows")
    }
    fn unminimize_main(&self) -> Result<(), DesktopError> {
        panic!("native read test does not unminimize")
    }
    fn focus_main(&self) -> Result<(), DesktopError> {
        panic!("native read test does not focus")
    }
    fn exit_desktop(&self) {
        panic!("native read test does not quit")
    }
}
impl CreditOpener for UnusedPorts {
    fn open(&self, _: CreditLink) -> Result<(), DesktopError> {
        panic!("native read test opens no links")
    }
}

pub struct Router {
    _app: tauri::App<MockRuntime>,
    window: tauri::WebviewWindow<MockRuntime>,
}
impl Router {
    pub fn new(bridge: DesktopBridge) -> Self {
        let ports = Arc::new(UnusedPorts);
        let app = arc_desktop::configure_with_ports(
            mock_builder(),
            bridge,
            DesktopPorts {
                shell: ports.clone(),
                opener: ports,
            },
        )
        .build(mock_context(noop_assets()))
        .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        Self { _app: app, window }
    }

    pub fn call(&self, command: &str, args: Value) -> Result<Value, Value> {
        tauri::test::get_ipc_response(
            &self.window,
            InvokeRequest {
                cmd: command.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.into(),
            },
        )
        .map(|body| body.deserialize().unwrap())
    }
}
