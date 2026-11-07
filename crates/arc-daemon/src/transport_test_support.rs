//! Test helpers shared by the owned transport/admission units only.

use crate::{
    admission::Requests, connection, limits::RuntimeLimits, runtime_metrics::RuntimeMetrics,
    transport,
};
use arc_ipc::{Request, Response};
use std::{io, sync::Arc, time::Duration};
use tokio::{net::UnixStream, time::Instant};

pub(crate) fn setup() -> (RuntimeLimits, Arc<RuntimeMetrics>) {
    let limits = RuntimeLimits::test_profile();
    let metrics = Arc::new(RuntimeMetrics::new(&limits));
    (limits, metrics)
}

pub(crate) async fn send(stream: &UnixStream, bytes: &[u8]) {
    let mut sent = 0;
    while sent < bytes.len() {
        stream.writable().await.unwrap();
        match stream.try_write(&bytes[sent..]) {
            Ok(n) => {
                assert_ne!(n, 0);
                sent += n;
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("{e}"),
        }
    }
}

pub(crate) async fn request(stream: &UnixStream, request: &Request) -> Response {
    let mut framed = Vec::new();
    arc_ipc::write_message(&mut framed, request).unwrap();
    send(stream, &framed).await;
    response(stream).await
}

pub(crate) async fn response(stream: &UnixStream) -> Response {
    let deadline = Instant::now() + Duration::from_secs(2);
    let declared = transport::read_header(stream, deadline).await.unwrap();
    let body = transport::read_body(stream, declared, deadline)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

pub(crate) fn spawn_handler(
    limits: RuntimeLimits,
    metrics: Arc<RuntimeMetrics>,
) -> (UnixStream, tokio::task::JoinHandle<()>) {
    let (server, client) = UnixStream::pair().unwrap();
    let requests = Requests::new(&limits, Arc::clone(&metrics)).unwrap();
    let task = tokio::spawn(connection::handle_connection(
        server,
        limits,
        requests,
        metrics,
        |_| async { panic!("fast path must not touch DB") },
    ));
    (client, task)
}
