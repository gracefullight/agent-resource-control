//! Response framing and timeout classification for admitted connections.
use crate::{
    limits::RuntimeLimits,
    runtime_metrics::{Counter, Gauge, RuntimeMetrics},
    transport,
};
use arc_ipc::{FrameError, Response, ERROR_FRAME_TOO_LARGE, MAX_FRAME_BYTES};
use std::{io, sync::Arc};
use tokio::{net::UnixStream, time::Instant};

pub(super) async fn respond(
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

pub(super) fn is_timeout(error: &FrameError) -> bool {
    matches!(error, FrameError::Io(error) if error.kind() == io::ErrorKind::TimedOut)
}
