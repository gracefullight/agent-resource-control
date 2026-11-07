use super::*;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Barrier,
    },
    thread,
    time::{Duration, Instant},
};

#[test]
fn maximum_and_exhausting_classification_load_preserves_health_and_shutdown() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn("");
    let max = format!("cargo test {}", "x".repeat(16384 - 11));
    assert_eq!(max.len(), 16384);
    let exhausted = format!("sh -c 'cargo test {}'", "x".repeat(12000));
    let complete = Arc::new(AtomicUsize::new(0));
    let running = Arc::new(AtomicBool::new(true));
    let barrier = Arc::new(Barrier::new(17));
    thread::scope(|scope| {
        for worker in 0..16 {
            let barrier = Arc::clone(&barrier);
            let running = Arc::clone(&running);
            let complete = Arc::clone(&complete);
            let socket = &fixture.socket;
            let max = &max;
            let exhausted = &exhausted;
            scope.spawn(move || {
                barrier.wait();
                let deadline = Instant::now() + Duration::from_secs(30);
                let mut round = worker;
                while running.load(Ordering::Acquire) && Instant::now() < deadline {
                    round += 1;
                    let Ok(mut stream) = UnixStream::connect(socket) else {
                        continue;
                    };
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let payload = json!({"command":if round%2 == 0 { max } else { exhausted }});
                    if let Ok(response) = arc_ipc::exchange(&mut stream, &request(payload)) {
                        assert_eq!(response.status, "ok");
                        let result = response.result.unwrap();
                        assert!(
                            result["reason"] == "CARGO_TEST" || result["reason"] == "WORK_LIMIT"
                        );
                        complete.fetch_add(1, Ordering::Release);
                    }
                }
            });
        }
        barrier.wait();
        let wait = Instant::now();
        while complete.load(Ordering::Acquire) < 16 {
            assert!(wait.elapsed() < Duration::from_secs(5));
            thread::yield_now();
        }
        let before_recovery = complete.load(Ordering::Acquire);
        let recovery_start = Instant::now();
        // Retry admission loss, but require a fixed malformed-frame error and
        // a valid classification on that same connection while workers run.
        loop {
            if let Ok(mut stream) = UnixStream::connect(&fixture.socket) {
                stream
                    .set_read_timeout(Some(Duration::from_millis(500)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_millis(500)))
                    .unwrap();
                if arc_ipc::write_frame(&mut stream, b"{").is_ok() {
                    if let Ok(response) = arc_ipc::read_response(&mut stream) {
                        assert_eq!(
                            response,
                            Response::error("", arc_ipc::ERROR_INVALID_REQUEST)
                        );
                        if let Ok(response) = arc_ipc::exchange(
                            &mut stream,
                            &request(json!({"command":"cargo test"})),
                        ) {
                            assert_eq!(response.status, "ok");
                            assert_eq!(response.result.unwrap()["reason"], "CARGO_TEST");
                            break;
                        }
                    }
                }
            }
            assert!(recovery_start.elapsed() < Duration::from_secs(2));
        }
        // Prove classification traffic continues across the recovery scenario.
        let at_recovery = complete.load(Ordering::Acquire);
        while complete.load(Ordering::Acquire) <= at_recovery {
            assert!(recovery_start.elapsed() < Duration::from_secs(2));
            thread::yield_now();
        }
        let recovery_us = recovery_start.elapsed().as_micros();
        let mut maximum_health_us = 0;
        // Admission rejection is allowed under saturation. Require actual health
        // successes within the existing deadline, without widening any cap.
        for _ in 0..10 {
            let start = Instant::now();
            loop {
                if let Ok(mut stream) = UnixStream::connect(&fixture.socket) {
                    stream
                        .set_read_timeout(Some(Duration::from_millis(500)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_millis(500)))
                        .unwrap();
                    if let Ok(response) = arc_ipc::exchange(
                        &mut stream,
                        &Request::new("health-under-classification", arc_ipc::OPERATION_HEALTH),
                    ) {
                        assert_eq!(response.status, "ok");
                        maximum_health_us = maximum_health_us.max(start.elapsed().as_micros());
                        break;
                    }
                }
                assert!(
                    start.elapsed() < Duration::from_secs(2),
                    "health starvation"
                );
            }
        }
        let (signal_us, _) = child.stop(libc::SIGTERM);
        running.store(false, Ordering::Release);
        assert!(signal_us <= 3_500_000);
        runtime::receipt(
            "classification-saturation",
            &json!({"completed":complete.load(Ordering::Acquire),"maximumHealthUs":maximum_health_us,"signalToExitUs":signal_us,"malformedRecoveryUs":recovery_us,"sameConnectionRecovery":true,"classificationsBeforeRecovery":before_recovery,"classificationsAtRecovery":at_recovery,"classificationLoadContinued":true}),
        );
    });
    let report = fixture.report();
    assert_eq!(report["limits"]["connections"], 64);
    assert_eq!(report["limits"]["requests"], 16);
    assert_eq!(report["metrics"]["db_outstanding"]["high_water"], 0);
    runtime::receipt("classification-accounting", &report);
}
