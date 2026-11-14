//! Exercise the production permit owners without racing the native IPC timeout.
use super::*;
use std::{
    future::Future,
    sync::mpsc,
    task::{Context, Poll, Wake, Waker},
    thread,
};

const WATCHDOG: Duration = Duration::from_secs(60);

fn bridge() -> DesktopBridge {
    DesktopBridge::with_socket_and_clock(PathBuf::from("unused-permit-test"), || 17)
}

fn governor_request() -> arc_ipc::governor::SetGovernorSwitchRequest {
    arc_ipc::governor::SetGovernorSwitchRequest {
        key: arc_ipc::governor::GovernorSwitchKey::GovernorEnabled,
        enabled: false,
        expected_revision: 4,
        mutation_id: format!("4-{}", "a".repeat(32)),
    }
}
fn busy(bridge: &DesktopBridge) {
    // All these public read families acquire the same production permit before IO.
    assert_eq!(bridge.daemon_health().unwrap_err().code, ErrorCode::Busy);
    assert_eq!(bridge.meter_snapshot().unwrap_err().code, ErrorCode::Busy);
    assert_eq!(bridge.policy_current().unwrap_err().code, ErrorCode::Busy);
    assert_eq!(bridge.recent_jobs().unwrap_err().code, ErrorCode::Busy);
    assert_eq!(
        bridge
            .governor_get_switch(Default::default())
            .unwrap_err()
            .code,
        ErrorCode::Busy
    );
    assert_eq!(
        bridge
            .governor_set_switch(governor_request())
            .unwrap_err()
            .code,
        ErrorCode::Rejected
    );
    assert_eq!(
        bridge
            .traffic_job_diagnostics(&"a".repeat(32))
            .unwrap_err()
            .code,
        ErrorCode::Busy
    );
    assert_eq!(
        tauri::async_runtime::block_on(bridge.governor_set_switch_async(governor_request()))
            .unwrap_err()
            .code,
        ErrorCode::Rejected
    );
    assert_eq!(
        tauri::async_runtime::block_on(bridge.traffic_job_diagnostics_async("a".repeat(32)))
            .unwrap_err()
            .code,
        ErrorCode::Busy
    );
    assert_eq!(
        tauri::async_runtime::block_on(bridge.meter_snapshot_async())
            .unwrap_err()
            .code,
        ErrorCode::Busy
    );
}

#[test]
fn sync_read_shares_exclusion_and_releases_after_success_error_and_unwind() {
    assert_eq!(READ_DEADLINE, Duration::from_secs(2));
    let bridge = bridge();
    for fail in [false, true] {
        let result = bridge.sync_read(|bridge| {
            busy(bridge);
            if fail {
                Err(DesktopError::new(ErrorCode::InvalidReply))
            } else {
                bridge.received(17)
            }
        });
        if fail {
            assert_eq!(result.unwrap_err().code, ErrorCode::InvalidReply);
        } else {
            let received = result.unwrap();
            assert_eq!((received.data, received.received_at_ms), (17, 17));
        }
        assert_eq!(bridge.sync_read(|_| Ok(23)).unwrap(), 23);
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), _> = bridge.sync_read(|_| panic!("owned read unwinds"));
    }));
    assert!(result.is_err());
    assert_eq!(bridge.sync_read(|_| Ok(29)).unwrap(), 29);
}

#[test]
fn blocking_read_keeps_shared_exclusion_until_release_for_success_and_error() {
    for fail in [false, true] {
        let bridge = bridge();
        thread::scope(|scope| {
            let (entered, ready) = mpsc::channel();
            let (release, released) = mpsc::channel();
            let owner = bridge.clone();
            let reader = scope.spawn(move || {
                tauri::async_runtime::block_on(owner.blocking(move |bridge| {
                    entered.send(()).unwrap();
                    released
                        .recv_timeout(WATCHDOG)
                        .map_err(|_| DesktopError::new(ErrorCode::Cancelled))?;
                    if fail {
                        Err(DesktopError::new(ErrorCode::InvalidReply))
                    } else {
                        bridge.received(31)
                    }
                }))
            });
            ready.recv_timeout(WATCHDOG).unwrap();
            busy(&bridge);
            release.send(()).unwrap();
            let result = reader.join().unwrap();
            if fail {
                assert_eq!(result.unwrap_err().code, ErrorCode::InvalidReply);
            } else {
                assert_eq!(result.unwrap().data, 31);
            }
            assert_eq!(bridge.sync_read(|_| Ok(37)).unwrap(), 37);
        });
    }
}

struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}

#[test]
fn dropped_async_waiter_retains_permit_until_blocking_owner_finishes() {
    let bridge = bridge();
    let (entered, ready) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let mut future = Box::pin(bridge.blocking(move |_| {
        entered.send(()).unwrap();
        released
            .recv_timeout(WATCHDOG)
            .map_err(|_| DesktopError::new(ErrorCode::Cancelled))
    }));
    let waker = Waker::from(Arc::new(Noop));
    assert!(matches!(
        future.as_mut().poll(&mut Context::from_waker(&waker)),
        Poll::Pending
    ));
    ready.recv_timeout(WATCHDOG).unwrap();
    drop(future);
    busy(&bridge);
    release.send(()).unwrap();
    // Observe the actual RAII release; the guard only detects a hung fixture.
    let end = Instant::now() + WATCHDOG;
    loop {
        match bridge.sync_read(|_| Ok(41)) {
            Ok(value) => {
                assert_eq!(value, 41);
                break;
            }
            Err(error) => assert_eq!(error.code, ErrorCode::Busy),
        }
        assert!(
            Instant::now() < end,
            "blocking owner failed to release permit"
        );
        thread::yield_now();
    }
}

#[test]
fn shutdown_cancels_in_flight_received_value_and_rejects_new_reads() {
    let bridge = bridge();
    thread::scope(|scope| {
        let (entered, ready) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let owner = bridge.clone();
        let reader = scope.spawn(move || {
            tauri::async_runtime::block_on(owner.blocking(move |bridge| {
                entered.send(()).unwrap();
                released
                    .recv_timeout(WATCHDOG)
                    .map_err(|_| DesktopError::new(ErrorCode::Cancelled))?;
                bridge.received(43)
            }))
        });
        ready.recv_timeout(WATCHDOG).unwrap();
        bridge.shutdown();
        assert_eq!(
            bridge
                .governor_set_switch(governor_request())
                .unwrap_err()
                .code,
            ErrorCode::Rejected
        );
        assert_eq!(
            bridge
                .traffic_job_diagnostics(&"a".repeat(32))
                .unwrap_err()
                .code,
            ErrorCode::Cancelled
        );
        assert_eq!(
            bridge.meter_snapshot().unwrap_err().code,
            ErrorCode::Cancelled
        );
        release.send(()).unwrap();
        assert_eq!(
            reader.join().unwrap().unwrap_err().code,
            ErrorCode::Cancelled
        );
        assert!(!bridge.0.in_flight.load(Ordering::Acquire));
        assert_eq!(
            tauri::async_runtime::block_on(bridge.daemon_health_async())
                .unwrap_err()
                .code,
            ErrorCode::Cancelled
        );
    });
}
