use super::*;

#[test]
fn accept_child() {
    let Some(mode) = std::env::var_os("ARC_ACCEPT_CHILD") else {
        return;
    };
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let (socket, _instance) = crate::prepare_instance().unwrap();
        let connection = arc_storage::open_and_migrate(arc_core::database_path().unwrap()).unwrap();
        let limits = RuntimeLimits::test_profile();
        let metrics = Arc::new(RuntimeMetrics::new(&limits));
        let worker = db_worker::DbWorker::start(connection, db_worker::DbConfig {
            queue_capacity: limits.db_queue, busy_timeout: limits.db_busy_timeout,
            execution_timeout: limits.db_execution_timeout,
        }, Arc::clone(&metrics)).unwrap();
        let (errno, count) = if mode == "once" { (libc::ECONNABORTED, 1) }
            else if mode == "stop" { (libc::EMFILE, usize::MAX) }
            else if mode == "fatal" { (libc::EBADF, 1) }
            else { (libc::EMFILE, 9) };
        let mut acceptor = crate::accept_recovery::Acceptor::injected(errno, count);
        if mode == "stop" { acceptor = acceptor.after_accepts(1); }
        let mut observed = acceptor.observe();
        let serving = crate::lifecycle::supervise_with_acceptor(socket, limits, Arc::clone(&metrics), worker, acceptor);
        tokio::pin!(serving);
        if mode == "stop" {
            loop {
                tokio::select! {
                    result = &mut serving => panic!("ended before backoff: {result:?}"),
                    changed = observed.changed() => {
                        changed.unwrap();
                        if *observed.borrow_and_update() >= 6 { break; }
                    }
                }
            }
            tokio::select! {
                result = &mut serving => panic!("ended before reaping: {result:?}"),
                _ = metrics.wait_for(|s| s.counter(crate::runtime_metrics::Counter::HandlersReaped) == 1) => {}
            }
            println!("accept-fixture: backing-off, handler-reaped");
        }
        let result = serving.await;
        if mode == "fatal" {
            assert_eq!(result.unwrap_err().downcast_ref::<std::io::Error>().unwrap().raw_os_error(), Some(libc::EBADF));
        } else { result.unwrap(); }

    });
}

fn cli_path() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("arcctl")
}

fn recovery_case(mode: &str) {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn_test(
        "lifecycle_tests::accept_recovery_tests::accept_child",
        "ARC_ACCEPT_CHILD",
        mode,
    );
    fixture.wait_for(&mut child, "arc-daemon ready.");
    for operation in ["status", "doctor"] {
        let output = Command::new(cli_path())
            .arg(operation)
            .env(arc_core::ARC_SOCKET_PATH, &fixture.socket)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode} {operation}: {output:?} {}",
            fixture.stdout()
        );
        assert!(output.stderr.is_empty(), "{output:?}");
    }
    stop(&mut child, libc::SIGTERM);
    let report = fixture.report();
    let errors = if mode == "once" { 1 } else { 9 };
    assert_eq!(report["supervisor"]["accept_errors"], errors);
    assert_eq!(report["supervisor"]["accept_retries"], errors);
    assert_eq!(report["supervisor"]["consecutive_errors"], 0);
    assert_eq!(
        report["supervisor"]["consecutive_errors_high_water"],
        errors
    );
    assert_eq!(report["supervisor"]["fatal_errors"], 0);
    assert_eq!(report["metrics"]["frames_completed"], 2);
    assert!(!fixture.stdout().contains("arc-daemon runtime:"));
    assert!(!read_bounded(&fixture.root.join("stderr")).contains("accept failed"));
    eprintln!("accept-recovery-receipt: {}", report);
}

#[test]
fn isolated_transient_accept_error_recovers_cli() {
    recovery_case("once");
}

#[test]
fn repeated_resource_pressure_recovers_cli() {
    recovery_case("pressure");
}

#[test]
fn signal_while_backing_off_is_accounted() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn_test(
        "lifecycle_tests::accept_recovery_tests::accept_child",
        "ARC_ACCEPT_CHILD",
        "stop",
    );
    fixture.wait_for(&mut child, "arc-daemon ready.");
    assert_eq!(
        arc_ipc::exchange(
            &mut fixture.connect(),
            &arc_ipc::Request::new("during-backoff", arc_ipc::OPERATION_HELLO)
        )
        .unwrap()
        .status,
        arc_ipc::STATUS_OK
    );
    fixture.wait_for(&mut child, "accept-fixture: backing-off");
    stop(&mut child, libc::SIGINT);
    let report = fixture.report();
    assert_eq!(report["reason"], "SIGINT");
    assert_eq!(report["supervisor"]["shutdown_during_backoff"], true);
    assert!(report["supervisor"]["accept_errors"].as_u64().unwrap() >= 6);
    assert_eq!(report["supervisor"]["backoff_high_water_ms"], 100);
    assert_eq!(report["metrics"]["handlers_spawned"], 1);
    assert_eq!(report["metrics"]["handlers_reaped"], 1);
    eprintln!("accept-backoff-signal-receipt: {}", report);
}

#[test]
fn unusable_listener_cause_survives_accounted_shutdown() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn_test(
        "lifecycle_tests::accept_recovery_tests::accept_child",
        "ARC_ACCEPT_CHILD",
        "fatal",
    );
    wait_exit(&mut child, true);
    let report = fixture.report();
    assert_eq!(report["reason"], "accept_error");
    assert_eq!(report["accept_error_os_code"], libc::EBADF);
    assert_eq!(report["supervisor"]["fatal_errors"], 1);
    assert_eq!(report["supervisor"]["accept_retries"], 0);
}
