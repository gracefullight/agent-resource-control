//! Frozen peers reject additive probes before exercising their legacy faults.
pub(super) fn unsupported_probe(request: &arc_ipc::Request) -> Option<arc_ipc::Response> {
    if !matches!(
        request.operation.as_str(),
        arc_ipc::policy::OPERATION_POLICY_CAPABILITIES | arc_ipc::governor::OPERATION_CAPABILITIES
    ) {
        return None;
    }
    assert!(request.payload.is_null());
    Some(arc_ipc::Response::error(
        &request.request_id,
        arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
    ))
}

pub(super) fn next_legacy(
    listener: &std::os::unix::net::UnixListener,
) -> (std::os::unix::net::UnixStream, arc_ipc::Request) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let (mut stream, _) = match listener.accept() {
            Ok(pair) => pair,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "legacy fixture admission deadline"
                );
                std::thread::yield_now();
                continue;
            }
            Err(e) => panic!("legacy accept: {e}"),
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
        if request.operation == arc_ipc::governor::OPERATION_CAPABILITIES {
            arc_ipc::write_response(&mut stream, &unsupported_probe(&request).unwrap()).unwrap();
            continue;
        }
        return (stream, request);
    }
}
