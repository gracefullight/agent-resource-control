use super::*;

#[test]
fn expired_ready_with_lost_negative_ack_never_retries_or_starts_a_target() {
    let socket = Socket::new();
    let path = socket.path();
    let server = thread::spawn(move || {
        let (mut stream, _) = socket.listener.accept().unwrap();
        let request = read(&mut stream);
        let mut policy = fixture::fallback();
        policy.observation = critical();
        policy.observation.pressure = Some(arc_core::meter::MemoryPressure::Normal);
        policy.observation.age_ms = policy.observation.stale_after_ms;
        policy.reason = arc_ipc::policy::PolicyReason::PressureNormal;
        reply(
            &mut stream,
            &request,
            json!(LaunchResult::Ready {
                permit_generation: 1,
                policy,
            }),
        );
        let ack = read(&mut stream);
        assert!(matches!(
            decode_launch_ack(&ack.payload, &input()).unwrap(),
            LaunchAck::Unspawned { .. }
        ));
        // No success response: retraction/receipt is uncertain to the CLI.
        drop(stream);
        socket.listener.set_nonblocking(true).unwrap();
        socket
    });
    assert!(matches!(
        wait(&path, input(), &Continuity::default(), &|| false, || {}),
        Err(Failure::Uncertain)
    ));
    let socket = server.join().unwrap();
    assert!(
        matches!(socket.listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock)
    );
}
