//! Sequential request handling; database policy is supplied by the worker adapter.

use crate::{
    admission::Requests,
    limits::RuntimeLimits,
    runtime_metrics::{Counter, Gauge, RuntimeMetrics},
    transport,
};
use arc_ipc::{
    DoctorReport, FrameError, Request, Response, ERROR_FRAME_TOO_LARGE, ERROR_INTEGRITY_FAILED,
    ERROR_INVALID_REQUEST, MAX_FRAME_BYTES, OPERATION_DOCTOR, PROTOCOL_VERSION,
};
use std::{future::Future, io, sync::Arc};
use tokio::{
    net::UnixStream,
    time::{timeout_at, Instant},
};

#[path = "connection/legacy_projection.rs"]
mod legacy_projection;
use legacy_projection::project_legacy_snapshot;

pub(crate) use arc_ipc::ERROR_DB_BUSY;
#[path = "policy_dispatch.rs"]
pub(crate) mod policy_dispatch;

#[cfg(test)]
#[path = "meter_dispatch_tests.rs"]
mod meter_tests;

/// A worker adapter maps typed DbError values without exposing SQLite to I/O.
pub(crate) enum DoctorOutcome {
    Ready(DoctorReport),
    Busy,
    IntegrityFailed,
}

#[cfg(test)]
pub(crate) async fn handle_connection<D, F>(
    stream: UnixStream,
    limits: RuntimeLimits,
    requests: Requests,
    metrics: Arc<RuntimeMetrics>,
    doctor: D,
) where
    D: Fn(std::time::Instant) -> F,
    F: Future<Output = DoctorOutcome>,
{
    handle_with_jobs(stream, limits, requests, metrics, doctor, |_, _| async {
        Err(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY)
    })
    .await;
}

#[cfg(test)]
pub(crate) async fn handle_with_jobs<D, F, J, G>(
    stream: UnixStream,
    limits: RuntimeLimits,
    requests: Requests,
    metrics: Arc<RuntimeMetrics>,
    doctor: D,
    jobs: J,
) where
    D: Fn(std::time::Instant) -> F,
    F: Future<Output = DoctorOutcome>,
    J: Fn(arc_ipc::jobs::LifecycleRequest, std::time::Instant) -> G,
    G: Future<Output = Result<serde_json::Value, &'static str>>,
{
    handle_inner(
        stream,
        limits,
        requests,
        metrics,
        doctor,
        jobs,
        TrafficHandler {
            enabled: false,
            handle: |_, _, _| async { Err(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY) },
            meter: None,
            meter_jobs: None,
            policy: None,
        },
    )
    .await;
}

#[cfg(test)]
#[allow(dead_code)] // The standalone Traffic target uses this legacy test adapter.
pub(crate) async fn handle_with_traffic<D, F, J, G, T, H>(
    stream: UnixStream,
    limits: RuntimeLimits,
    requests: Requests,
    metrics: Arc<RuntimeMetrics>,
    doctor: D,
    jobs: J,
    traffic: T,
) where
    D: Fn(std::time::Instant) -> F,
    F: Future<Output = DoctorOutcome>,
    J: Fn(arc_ipc::jobs::LifecycleRequest, std::time::Instant) -> G,
    G: Future<Output = Result<serde_json::Value, &'static str>>,
    T: Fn(arc_ipc::traffic::JobRequest, u32, std::time::Instant) -> H,
    H: Future<Output = Result<serde_json::Value, &'static str>>,
{
    handle_with_meter(
        stream,
        limits,
        requests,
        metrics,
        doctor,
        jobs,
        (traffic, None),
    )
    .await;
}

#[cfg(test)]
pub(crate) async fn handle_with_meter<D, F, J, G, T, H>(
    stream: UnixStream,
    limits: RuntimeLimits,
    requests: Requests,
    metrics: Arc<RuntimeMetrics>,
    doctor: D,
    jobs: J,
    traffic: (T, Option<arc_meter::SnapshotReader>),
) where
    D: Fn(std::time::Instant) -> F,
    F: Future<Output = DoctorOutcome>,
    J: Fn(arc_ipc::jobs::LifecycleRequest, std::time::Instant) -> G,
    G: Future<Output = Result<serde_json::Value, &'static str>>,
    T: Fn(arc_ipc::traffic::JobRequest, u32, std::time::Instant) -> H,
    H: Future<Output = Result<serde_json::Value, &'static str>>,
{
    handle_with_job_meter(
        stream,
        limits,
        requests,
        metrics,
        doctor,
        jobs,
        (traffic.0, traffic.1, None),
    )
    .await;
}

#[cfg(test)]
pub(crate) async fn handle_with_job_meter<D, F, J, G, T, H>(
    stream: UnixStream,
    limits: RuntimeLimits,
    requests: Requests,
    metrics: Arc<RuntimeMetrics>,
    doctor: D,
    jobs: J,
    traffic: (
        T,
        Option<arc_meter::SnapshotReader>,
        Option<crate::job_meter_dispatch::Handler>,
    ),
) where
    D: Fn(std::time::Instant) -> F,
    F: Future<Output = DoctorOutcome>,
    J: Fn(arc_ipc::jobs::LifecycleRequest, std::time::Instant) -> G,
    G: Future<Output = Result<serde_json::Value, &'static str>>,
    T: Fn(arc_ipc::traffic::JobRequest, u32, std::time::Instant) -> H,
    H: Future<Output = Result<serde_json::Value, &'static str>>,
{
    handle_with_policy(
        stream, limits, requests, metrics, doctor, jobs, traffic, None,
    )
    .await;
}

#[path = "policy_connection.rs"]
mod policy_connection;
pub(crate) use policy_connection::handle_with_policy;

struct TrafficHandler<T> {
    enabled: bool,
    handle: T,
    meter: Option<arc_meter::SnapshotReader>,
    meter_jobs: Option<crate::job_meter_dispatch::Handler>,
    policy: Option<policy_dispatch::Handler>,
}

async fn handle_inner<D, F, J, G, T, H>(
    stream: UnixStream,
    limits: RuntimeLimits,
    requests: Requests,
    metrics: Arc<RuntimeMetrics>,
    doctor: D,
    jobs: J,
    traffic: TrafficHandler<T>,
) where
    D: Fn(std::time::Instant) -> F,
    F: Future<Output = DoctorOutcome>,
    J: Fn(arc_ipc::jobs::LifecycleRequest, std::time::Instant) -> G,
    G: Future<Output = Result<serde_json::Value, &'static str>>,
    T: Fn(arc_ipc::traffic::JobRequest, u32, std::time::Instant) -> H,
    H: Future<Output = Result<serde_json::Value, &'static str>>,
{
    loop {
        let start = Instant::now();
        let declared = match transport::read_header(&stream, start + limits.header_timeout).await {
            Ok(declared) => declared,
            Err(error @ (FrameError::EmptyFrame | FrameError::TooLarge { .. })) => {
                let Some(_admission) = requests.try_admit() else {
                    return;
                };
                let code = if matches!(error, FrameError::TooLarge { .. }) {
                    ERROR_FRAME_TOO_LARGE
                } else {
                    ERROR_INVALID_REQUEST
                };
                let response = Response::error("", code);
                respond(&stream, &response, &limits, &metrics).await;
                return;
            }
            Err(error) => {
                if is_timeout(&error) {
                    metrics.increment(Counter::HeaderTimeouts);
                }
                return;
            }
        };
        let Some(_admission) = requests.try_admit() else {
            return;
        };
        let body_guard = metrics.enter(Gauge::RequestBodyBytes, declared as usize);
        let body = match transport::read_body(&stream, declared, start + limits.frame_timeout).await
        {
            Ok(body) => body,
            Err(error) => {
                if is_timeout(&error) {
                    metrics.increment(Counter::FrameTimeouts);
                }
                return;
            }
        };
        let decoded = Request::decode(&body);
        drop(body);
        drop(body_guard);
        let mut request = match decoded {
            Ok(request) => request,
            Err(_) => {
                if !respond(
                    &stream,
                    &Response::error("", ERROR_INVALID_REQUEST),
                    &limits,
                    &metrics,
                )
                .await
                {
                    return;
                }
                continue;
            }
        };
        if policy_dispatch::dispatch(
            &stream,
            &request,
            &limits,
            traffic.policy.as_ref(),
            &metrics,
        )
        .await
        {
            continue;
        }
        // Classification consumes the payload synchronously under request admission.
        // Version rejection in dispatch precedes all typed payload decoding.
        let classified = if request.operation == arc_ipc::OPERATION_JOB_CLASSIFY {
            Some(arc_ipc::dispatch(&request, || {
                unreachable!("classification never uses SQLite")
            }))
        } else {
            None
        };
        let lifecycle = if request.protocol_version == PROTOCOL_VERSION {
            arc_ipc::jobs::LifecycleRequest::decode(&request.operation, &request.payload)
        } else {
            None
        };
        let traffic_probe = traffic.enabled
            && request.protocol_version == PROTOCOL_VERSION
            && matches!(
                request.operation.as_str(),
                arc_ipc::traffic::OPERATION_TRAFFIC_CAPABILITIES
                    | arc_ipc::traffic::OPERATION_BUN_CAPABILITIES
            );
        let probe_valid = traffic_probe && arc_ipc::traffic::decode_probe(&request.payload).is_ok();
        let traffic_job = if traffic.enabled
            && request.protocol_version == PROTOCOL_VERSION
            && matches!(
                request.operation.as_str(),
                arc_ipc::traffic::OPERATION_TRAFFIC_JOB | arc_ipc::traffic::OPERATION_BUN_JOB
            ) {
            Some(arc_ipc::traffic::decode_job_for(
                &request.operation,
                &request.payload,
            ))
        } else {
            None
        };
        // Neither the body nor payload enters the worker or survives an await.
        let metered = if request.protocol_version == PROTOCOL_VERSION
            && request.operation == arc_ipc::meter::OPERATION_METER_SNAPSHOT
        {
            Some(crate::meter::response(&request, traffic.meter.as_ref()))
        } else {
            None
        };
        let job_meter =
            if traffic.meter_jobs.is_some() && request.protocol_version == PROTOCOL_VERSION {
                arc_ipc::meter::JobMeterRequest::decode(&request.operation, &request.payload)
            } else {
                None
            };
        request.payload = serde_json::Value::Null;
        let response = if let Some(input) = job_meter {
            crate::job_meter_dispatch::response(
                traffic.meter_jobs.as_ref().expect("enabled Meter"),
                &stream,
                &request.request_id,
                input,
                &limits,
                &metrics,
            )
            .await
        } else if let Some(response) = metered {
            response
        } else if traffic_probe {
            if probe_valid {
                Response::ok(
                    &request.request_id,
                    serde_json::to_value(
                        if request.operation == arc_ipc::traffic::OPERATION_BUN_CAPABILITIES {
                            arc_ipc::traffic::Capabilities::bun_supported()
                        } else {
                            arc_ipc::traffic::Capabilities::supported()
                        },
                    )
                    .expect("serializable capability"),
                )
            } else {
                Response::error(&request.request_id, ERROR_INVALID_REQUEST)
            }
        } else if let Some(input) = traffic_job {
            match (input, crate::platform::peer_pid(&stream)) {
                (Err(code), _) => Response::error(&request.request_id, code),
                (_, Err(_)) => Response::error(&request.request_id, ERROR_INVALID_REQUEST),
                (Ok(input), Ok(peer)) => {
                    #[cfg(test)]
                    let bun_enqueue = request.operation == arc_ipc::traffic::OPERATION_BUN_JOB
                        && matches!(&input, arc_ipc::traffic::JobRequest::Enqueue { .. });
                    let deadline = std::time::Instant::now() + limits.db_response_timeout;
                    match timeout_at(
                        Instant::from_std(deadline),
                        (traffic.handle)(input, peer, deadline),
                    )
                    .await
                    {
                        Ok(Ok(value)) => {
                            #[cfg(test)]
                            if bun_enqueue
                                && value.get("state").and_then(serde_json::Value::as_str)
                                    == Some("GRANTED")
                                && drop_bun_grant_for_test(peer)
                            {
                                return;
                            }
                            Response::ok(&request.request_id, value)
                        }
                        Ok(Err(code)) => Response::error(
                            &request.request_id,
                            if request.operation == arc_ipc::traffic::OPERATION_BUN_JOB
                                && code == arc_ipc::ERROR_UNSUPPORTED_CAPABILITY
                            {
                                ERROR_INVALID_REQUEST
                            } else {
                                code
                            },
                        ),
                        Err(_) => {
                            metrics.increment(Counter::DbTimeouts);
                            Response::error(&request.request_id, ERROR_DB_BUSY)
                        }
                    }
                }
            }
        } else if let Some(input) = lifecycle {
            match input {
                Err(code) => Response::error(&request.request_id, code),
                Ok(input) => {
                    let deadline = std::time::Instant::now() + limits.db_response_timeout;
                    match timeout_at(Instant::from_std(deadline), jobs(input, deadline)).await {
                        Ok(Ok(mut value)) => {
                            project_legacy_snapshot(&mut value);
                            Response::ok(&request.request_id, value)
                        }
                        Ok(Err(code)) => Response::error(&request.request_id, code),
                        Err(_) => {
                            metrics.increment(Counter::DbTimeouts);
                            Response::error(&request.request_id, ERROR_DB_BUSY)
                        }
                    }
                }
            }
        } else if let Some(response) = classified {
            response
        } else if request.protocol_version == PROTOCOL_VERSION
            && request.operation == OPERATION_DOCTOR
        {
            let deadline = std::time::Instant::now() + limits.db_response_timeout;
            match timeout_at(Instant::from_std(deadline), doctor(deadline)).await {
                Ok(DoctorOutcome::Ready(report)) => arc_ipc::dispatch(&request, || Ok(report)),
                Ok(DoctorOutcome::IntegrityFailed) => {
                    Response::error(&request.request_id, ERROR_INTEGRITY_FAILED)
                }
                outcome => {
                    if outcome.is_err() {
                        metrics.increment(Counter::DbTimeouts);
                    }
                    Response::error(&request.request_id, ERROR_DB_BUSY)
                }
            }
        } else {
            arc_ipc::dispatch(&request, || {
                unreachable!("non-doctor dispatch cannot use the database")
            })
        };
        if !respond(&stream, &response, &limits, &metrics).await {
            return;
        }
    }
}

// Only the test daemon subprocess enables this fault. Stop its authenticated
// CLI after the worker committed and close the connection without the reply.
#[cfg(test)]
fn drop_bun_grant_for_test(peer: u32) -> bool {
    let Some(marker) = std::env::var_os("ARC_TEST_BUN_GRANT_ACK_LOST") else {
        return false;
    };
    assert_eq!(unsafe { libc::kill(peer as i32, libc::SIGSTOP) }, 0);
    std::fs::write(marker, b"GRANTED").expect("write committed-grant test marker");
    true
}

async fn respond(
    stream: &UnixStream,
    response: &Response,
    limits: &RuntimeLimits,
    metrics: &Arc<RuntimeMetrics>,
) -> bool {
    let deadline = Instant::now() + limits.write_timeout;
    let _bytes = metrics.enter(Gauge::ResponseBytes, MAX_FRAME_BYTES as usize);
    let result = transport::write_response(stream, response, deadline).await;
    match result {
        Ok(()) => {
            metrics.increment(Counter::FramesCompleted);
            true
        }
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            // Failed serialization has been dropped. The fallback shares its deadline.
            let fallback = Response::error("", ERROR_FRAME_TOO_LARGE);
            match transport::write_response(stream, &fallback, deadline).await {
                Ok(()) => metrics.increment(Counter::FramesCompleted),
                Err(error) if error.kind() == io::ErrorKind::TimedOut => {
                    metrics.increment(Counter::WriteTimeouts)
                }
                Err(_) => {}
            }
            false
        }
        Err(error) => {
            if error.kind() == io::ErrorKind::TimedOut {
                metrics.increment(Counter::WriteTimeouts);
            }
            false
        }
    }
}

fn is_timeout(error: &FrameError) -> bool {
    matches!(error, FrameError::Io(error) if error.kind() == io::ErrorKind::TimedOut)
}
