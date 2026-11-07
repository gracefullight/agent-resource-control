use crate::{
    runtime::{receipt, Fixture},
    spawn, support,
};
use std::{
    io::Write,
    time::{Duration, Instant},
};

#[test]
fn full_request_and_connection_saturation_recovers_without_restart() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = spawn(&fixture, "deadlines");
    let start = Instant::now();
    let mut bodies = Vec::new();
    for _ in 0..4 {
        let mut peer = fixture.connect();
        peer.write_all(&1024u32.to_be_bytes()).unwrap();
        peer.write_all(b"{").unwrap();
        bodies.push(peer);
    }
    fixture.wait_for(&mut child, "bounded-child:requests-full");
    let rejected = fixture.cli("status");
    assert!(!rejected.status.success(), "{rejected:?}");
    assert_eq!(rejected.stderr, b"DAEMON_UNAVAILABLE\n");
    assert!(rejected.stdout.is_empty());
    let mut headers = Vec::new();
    for _ in 0..4 {
        let mut peer = fixture.connect();
        peer.write_all(&[0]).unwrap();
        headers.push(peer);
    }
    fixture.wait_for(&mut child, "bounded-child:connections-full");
    let rejected = fixture.cli("doctor");
    assert!(!rejected.status.success(), "{rejected:?}");
    assert_eq!(rejected.stderr, b"DAEMON_UNAVAILABLE\n");
    assert!(rejected.stdout.is_empty());
    fixture.wait_for(&mut child, "bounded-child:deadline-released");
    let recovery_us = start.elapsed().as_micros();
    // 300 ms whole-frame deadline plus a disclosed 250 ms native margin.
    assert!(start.elapsed() <= Duration::from_millis(550));
    fixture.assert_cli();
    drop(bodies);
    drop(headers);
    child.stop(libc::SIGTERM);
    let report = fixture.report();
    for (key, count) in [
        ("header_timeouts", 4),
        ("frame_timeouts", 4),
        ("request_rejected", 1),
        ("connection_rejected", 1),
    ] {
        assert_eq!(report["metrics"][key], count, "{report}");
    }
    assert_eq!(report["metrics"]["connections"]["high_water"], 8);
    assert_eq!(report["metrics"]["tracked_tasks"]["high_water"], 8);
    assert_eq!(report["metrics"]["requests"]["high_water"], 4);
    receipt(
        "deadline-saturation",
        &serde_json::json!({"report":report,"recovery_us":recovery_us,
        "recovered_same_process":true,"profile":"test","scheduling_margin_ms":250}),
    );
}
