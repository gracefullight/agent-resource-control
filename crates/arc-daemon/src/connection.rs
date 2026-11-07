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

pub(crate) use arc_ipc::ERROR_DB_BUSY;

/// A worker adapter maps typed DbError values without exposing SQLite to I/O.
pub(crate) enum DoctorOutcome {
    Ready(DoctorReport),
    Busy,
    IntegrityFailed,
}

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
        // Classification consumes the payload synchronously under request admission.
        // Version rejection in dispatch precedes all typed payload decoding.
        let classified = if request.operation == arc_ipc::OPERATION_JOB_CLASSIFY {
            Some(arc_ipc::dispatch(&request, || {
                unreachable!("classification never uses SQLite")
            }))
        } else {
            None
        };
        // Neither the body nor payload enters the worker or survives an await.
        request.payload = serde_json::Value::Null;
        let response = if let Some(response) = classified {
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
