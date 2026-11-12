//! The production read bridge used by both Tauri handlers and native probes.
use crate::{
    dto::*,
    error::{DesktopError, ErrorCode},
};
use arc_ipc::{
    jobs::{JobId, StatusRequest, StatusResult, MAX_TIMESTAMP, OPERATION_JOB_STATUS},
    Request,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const READ_DEADLINE: Duration = Duration::from_secs(2);

enum Endpoint {
    System,
    Socket(PathBuf),
}

struct Inner {
    endpoint: Endpoint,
    in_flight: AtomicBool,
    cancelled: AtomicBool,
    clock: Box<dyn Fn() -> u64 + Send + Sync>,
}

#[derive(Clone)]
pub struct DesktopBridge(Arc<Inner>);

impl std::fmt::Debug for DesktopBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopBridge")
            .field("endpoint", &"<redacted>")
            .field("cancelled", &self.is_shutdown())
            .finish()
    }
}

struct Permit(Arc<Inner>);
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.in_flight.store(false, Ordering::Release);
    }
}

impl Default for DesktopBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopBridge {
    /// Resolves the same production endpoint as the CLI for each read.
    pub fn new() -> Self {
        Self::build(Endpoint::System, wall_clock_ms)
    }

    /// Native-only fixture/configuration seam. JS cannot choose an endpoint.
    pub fn from_socket_path(socket: PathBuf) -> Self {
        Self::build(Endpoint::Socket(socket), wall_clock_ms)
    }

    /// Deterministic receipt clock for cross-language handler fixtures.
    pub fn with_socket_and_clock(
        socket: PathBuf,
        clock: impl Fn() -> u64 + Send + Sync + 'static,
    ) -> Self {
        Self::build(Endpoint::Socket(socket), clock)
    }

    fn build(endpoint: Endpoint, clock: impl Fn() -> u64 + Send + Sync + 'static) -> Self {
        Self(Arc::new(Inner {
            endpoint,
            in_flight: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            clock: Box::new(clock),
        }))
    }

    pub fn shutdown(&self) {
        self.0.cancelled.store(true, Ordering::Release);
    }
    pub fn is_shutdown(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }

    fn permit(&self) -> Result<Permit, DesktopError> {
        if self.is_shutdown() {
            return Err(DesktopError::new(ErrorCode::Cancelled));
        }
        self.0
            .in_flight
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map_err(|_| DesktopError::new(ErrorCode::Busy))?;
        let permit = Permit(self.0.clone());
        if self.is_shutdown() {
            return Err(DesktopError::new(ErrorCode::Cancelled));
        }
        Ok(permit)
    }

    fn sync_read<T>(
        &self,
        read: impl FnOnce(&Self) -> Result<T, DesktopError>,
    ) -> Result<T, DesktopError> {
        let _permit = self.permit()?;
        read(self)
    }

    // Acquire before queueing: concurrent invokes never accumulate socket jobs.
    // The permit belongs to the blocking task even if its future is dropped.
    async fn blocking<T: Send + 'static>(
        &self,
        read: impl FnOnce(&Self) -> Result<T, DesktopError> + Send + 'static,
    ) -> Result<T, DesktopError> {
        let permit = self.permit()?;
        let bridge = self.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let _permit = permit;
            read(&bridge)
        })
        .await
        .map_err(|_| DesktopError::new(ErrorCode::Local))?
    }

    pub fn daemon_health(&self) -> Result<Received<DaemonHealth>, DesktopError> {
        self.sync_read(Self::read_health)
    }
    pub fn meter_snapshot(&self) -> Result<Received<MeterSnapshot>, DesktopError> {
        self.sync_read(Self::read_meter)
    }
    pub async fn meter_snapshot_async(&self) -> Result<Received<MeterSnapshot>, DesktopError> {
        self.blocking(Self::read_meter).await
    }
    pub fn recent_jobs(&self) -> Result<Received<RecentJobs>, DesktopError> {
        self.sync_read(Self::read_recent)
    }
    pub fn job_by_id(&self, job_id: &str) -> Result<Received<JobLookup>, DesktopError> {
        let id = validate_job_id(job_id)?;
        self.sync_read(|bridge| bridge.read_job(id))
    }
    pub async fn daemon_health_async(&self) -> Result<Received<DaemonHealth>, DesktopError> {
        self.blocking(Self::read_health).await
    }
    pub async fn recent_jobs_async(&self) -> Result<Received<RecentJobs>, DesktopError> {
        self.blocking(Self::read_recent).await
    }
    pub async fn job_by_id_async(
        &self,
        job_id: String,
    ) -> Result<Received<JobLookup>, DesktopError> {
        let id = validate_job_id(&job_id)?;
        self.blocking(move |bridge| bridge.read_job(id)).await
    }

    fn received<T>(&self, data: T) -> Result<Received<T>, DesktopError> {
        if self.is_shutdown() {
            return Err(DesktopError::new(ErrorCode::Cancelled));
        }
        let received_at_ms = (self.0.clock)();
        if received_at_ms > MAX_TIMESTAMP {
            return Err(DesktopError::new(ErrorCode::Local));
        }
        Ok(Received {
            data,
            received_at_ms,
        })
    }

    fn exchange(&self, operation: &'static str, payload: Value) -> Result<Value, DesktopError> {
        #[cfg(unix)]
        {
            use arc_ipc::client::{exchange_until, ExchangeOptions};
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let deadline = Instant::now() + READ_DEADLINE;
            let socket = match &self.0.endpoint {
                Endpoint::System => {
                    arc_core::socket_path().map_err(|_| DesktopError::new(ErrorCode::Local))?
                }
                Endpoint::Socket(socket) => socket.clone(),
            };
            let mut request = Request::new(
                format!(
                    "desktop-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ),
                operation,
            );
            request.payload = payload;
            exchange_until(
                &socket,
                &request,
                deadline,
                &|| self.is_shutdown(),
                &mut false,
                ExchangeOptions {
                    continuity: None,
                    retry_busy: operation != arc_ipc::meter::OPERATION_METER_SNAPSHOT,
                },
            )
            .map_err(Into::into)
        }
        #[cfg(not(unix))]
        {
            let _ = (operation, payload);
            Err(DesktopError::new(ErrorCode::Unsupported))
        }
    }

    fn read_health(&self) -> Result<Received<DaemonHealth>, DesktopError> {
        let value = self.exchange(arc_ipc::OPERATION_HEALTH, Value::Null)?;
        if !value.is_object() {
            return Err(DesktopError::new(ErrorCode::InvalidReply));
        }
        let health: DaemonHealth = serde_json::from_value(value)
            .map_err(|_| DesktopError::new(ErrorCode::InvalidReply))?;
        if health.protocol_version != arc_ipc::PROTOCOL_VERSION {
            return Err(DesktopError::new(ErrorCode::Protocol));
        }
        self.received(health)
    }

    fn read_meter(&self) -> Result<Received<MeterSnapshot>, DesktopError> {
        let value = self.exchange(arc_ipc::meter::OPERATION_METER_SNAPSHOT, Value::Null)?;
        let snapshot = arc_ipc::meter::decode_snapshot(&value)
            .map_err(|_| DesktopError::new(ErrorCode::InvalidReply))?;
        self.received(snapshot)
    }

    fn status(&self, query: StatusRequest) -> Result<StatusResult, DesktopError> {
        let value = self.exchange(OPERATION_JOB_STATUS, json!(query))?;
        // serde can accept positional arrays for some structs/enum variants.
        // Protocol objects remain objects even if such a value could decode to
        // a structurally valid Rust snapshot. Check before dropping any fields.
        if !status_has_object_shapes(&value) {
            return Err(DesktopError::new(ErrorCode::InvalidReply));
        }
        let status: StatusResult = serde_json::from_value(value)
            .map_err(|_| DesktopError::new(ErrorCode::InvalidReply))?;
        if !status.is_valid() {
            return Err(DesktopError::new(ErrorCode::InvalidReply));
        }
        Ok(status)
    }

    fn read_recent(&self) -> Result<Received<RecentJobs>, DesktopError> {
        let StatusResult::Recent {
            jobs,
            truncated,
            observed_at_ms,
            liveness_fresh,
        } = self.status(StatusRequest::Recent {})?
        else {
            return Err(DesktopError::new(ErrorCode::InvalidReply));
        };
        let unique: BTreeSet<_> = jobs.iter().map(|job| job.job_id.as_str()).collect();
        if unique.len() != jobs.len() {
            return Err(DesktopError::new(ErrorCode::InvalidReply));
        }
        self.received(RecentJobs {
            jobs,
            truncated,
            observed_at_ms,
            liveness_fresh,
            limit: 50,
        })
    }

    fn read_job(&self, job_id: JobId) -> Result<Received<JobLookup>, DesktopError> {
        let lookup = match self.status(StatusRequest::ById {
            job_id: job_id.clone(),
        })? {
            StatusResult::Found { job } if job.job_id == job_id => {
                JobLookup::Found { job: (*job).into() }
            }
            StatusResult::NotFound {} => JobLookup::NotFound {},
            _ => return Err(DesktopError::new(ErrorCode::InvalidReply)),
        };
        self.received(lookup)
    }
}

fn validate_job_id(value: &str) -> Result<JobId, DesktopError> {
    // Reject oversize input before copying or doing socket work.
    if value.len() != 32 {
        return Err(DesktopError::new(ErrorCode::InvalidRequest));
    }
    JobId::try_from(value.to_owned()).map_err(|_| DesktopError::new(ErrorCode::InvalidRequest))
}

fn wall_clock_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| duration.as_millis().try_into().ok())
        .unwrap_or(MAX_TIMESTAMP + 1)
}

fn status_has_object_shapes(value: &Value) -> bool {
    if !value.is_object() {
        return false;
    }
    match value.get("kind").and_then(Value::as_str) {
        Some("FOUND") => {
            let Some(job) = value.get("job").and_then(Value::as_object) else {
                return false;
            };
            ["classification", "observer", "observation"]
                .iter()
                .all(|field| job.get(*field).is_some_and(Value::is_object))
                && ["child", "outcome"].iter().all(|field| {
                    job.get(*field)
                        .is_some_and(|value| value.is_null() || value.is_object())
                })
        }
        Some("RECENT") => value
            .get("jobs")
            .and_then(Value::as_array)
            .is_some_and(|jobs| jobs.len() <= 50 && jobs.iter().all(Value::is_object)),
        Some("NOT_FOUND") => true,
        _ => false,
    }
}
