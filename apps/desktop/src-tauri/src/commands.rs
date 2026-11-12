//! Fixed registered operations; no JS operation/payload, SQL or execution proxy.
use crate::{
    dto::{self, *},
    error::DesktopError,
    lifecycle::{CreditLink, DesktopState, LifecycleEvent},
};
use tauri::State;

#[tauri::command]
pub async fn meter_snapshot(
    state: State<'_, DesktopState>,
) -> Result<Received<MeterSnapshot>, DesktopError> {
    state.bridge.meter_snapshot_async().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn meter_summary(
    state: State<'_, DesktopState>,
    job_id: String,
) -> Result<Received<JobMeterSummary>, DesktopError> {
    state.bridge.meter_summary_async(job_id).await
}

#[tauri::command]
pub async fn daemon_health(
    state: State<'_, DesktopState>,
) -> Result<Received<DaemonHealth>, DesktopError> {
    state.bridge.daemon_health_async().await
}

#[tauri::command]
pub async fn recent_jobs(
    state: State<'_, DesktopState>,
) -> Result<Received<RecentJobs>, DesktopError> {
    state.bridge.recent_jobs_async().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn job_by_id(
    state: State<'_, DesktopState>,
    job_id: String,
) -> Result<Received<JobLookup>, DesktopError> {
    state.bridge.job_by_id_async(job_id).await
}

#[tauri::command]
pub fn desktop_metadata() -> Result<DesktopMetadata, DesktopError> {
    Ok(dto::desktop_metadata())
}

#[tauri::command(rename_all = "snake_case")]
pub fn open_credit_link(state: State<'_, DesktopState>, link: String) -> Result<(), DesktopError> {
    state.open_credit_link(CreditLink::parse(&link)?)
}

#[tauri::command]
pub fn open_dashboard(state: State<'_, DesktopState>) -> Result<(), DesktopError> {
    state.dispatch(LifecycleEvent::OpenDashboard)
}
#[tauri::command]
pub fn hide_dashboard(state: State<'_, DesktopState>) -> Result<(), DesktopError> {
    state.dispatch(LifecycleEvent::CloseRequested)
}
#[tauri::command]
pub fn quit_desktop(state: State<'_, DesktopState>) -> Result<(), DesktopError> {
    state.dispatch(LifecycleEvent::QuitDesktop)
}
