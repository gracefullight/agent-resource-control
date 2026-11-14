pub mod bridge;
pub mod commands;
pub mod dto;
pub mod error;
pub mod lifecycle;

#[cfg(all(test, unix))]
mod meter_bridge_tests;

use bridge::DesktopBridge;
use lifecycle::{DesktopPorts, DesktopState, TauriCreditOpener, TauriShell};
use std::sync::Arc;
use tauri::{Manager, Runtime};

/// Install the production router, native menu/window callbacks and official
/// opener. No daemon start/stop or database lifecycle is owned by this client.
pub fn configure<R: Runtime>(
    builder: tauri::Builder<R>,
    bridge: DesktopBridge,
) -> tauri::Builder<R> {
    install(
        builder,
        bridge,
        |app| DesktopPorts {
            shell: Arc::new(TauriShell(app.clone())),
            opener: Arc::new(TauriCreditOpener(app.clone())),
        },
        true,
    )
}

/// Supported headless test seam: only OS ports and tray creation differ. The
/// router and registered callbacks are the same as production Wry's.
pub fn configure_with_ports<R: Runtime>(
    builder: tauri::Builder<R>,
    bridge: DesktopBridge,
    ports: DesktopPorts,
) -> tauri::Builder<R> {
    install(builder, bridge, move |_| ports, false)
}

fn install<R: Runtime>(
    builder: tauri::Builder<R>,
    bridge: DesktopBridge,
    ports: impl FnOnce(&tauri::AppHandle<R>) -> DesktopPorts + Send + 'static,
    native_tray: bool,
) -> tauri::Builder<R> {
    builder
        .plugin(
            tauri_plugin_opener::Builder::new()
                .open_js_links_on_click(false)
                .build(),
        )
        .plugin(
            tauri::plugin::Builder::<R, ()>::new("arc-desktop-state")
                .setup(move |app, _| {
                    app.manage(DesktopState::new(bridge, ports(app)));
                    Ok(())
                })
                .build(),
        )
        .setup(move |app| {
            if native_tray {
                lifecycle::install_tray(app.handle())?;
            }
            Ok(())
        })
        .on_window_event(lifecycle::on_window_event)
        .on_menu_event(lifecycle::on_menu_event)
        .on_tray_icon_event(lifecycle::on_tray_icon_event)
        .invoke_handler(tauri::generate_handler![
            commands::governor_get_switch,
            commands::governor_set_switch,
            commands::daemon_health,
            commands::meter_snapshot,
            commands::meter_summary,
            commands::policy_current,
            commands::policy_job,
            commands::traffic_job_diagnostics,
            commands::recent_jobs,
            commands::job_by_id,
            commands::desktop_metadata,
            commands::open_credit_link,
            commands::open_dashboard,
            commands::hide_dashboard,
            commands::quit_desktop,
        ])
}

pub fn run() {
    let app = configure(tauri::Builder::default(), DesktopBridge::new())
        .build(tauri::generate_context!())
        .unwrap_or_else(|_| {
            eprintln!("ARC desktop runtime could not start");
            std::process::exit(1);
        });
    app.run(lifecycle::on_run_event);
}
