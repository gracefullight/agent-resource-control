use super::*;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};

#[test]
fn report_child() {
    let Some(mode) = std::env::var_os("ARC_REPORT_CHILD") else {
        return;
    };
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let (socket, _instance) = crate::prepare_instance().unwrap();
            let connection =
                arc_storage::open_and_migrate(arc_core::database_path().unwrap()).unwrap();
            let limits = RuntimeLimits::test_profile();
            let metrics = Arc::new(RuntimeMetrics::new(&limits));
            let worker = db_worker::DbWorker::start(
                connection,
                db_worker::DbConfig {
                    queue_capacity: limits.db_queue,
                    busy_timeout: limits.db_busy_timeout,
                    execution_timeout: limits.db_execution_timeout,
                },
                Arc::clone(&metrics),
            )
            .unwrap();
            let result = if mode == "write-failure" {
                let path = socket.with_file_name("write-failure");
                fs::write(&path, b"unchanged").unwrap();
                crate::lifecycle::supervise_with_report(
                    socket.clone(),
                    limits,
                    Arc::clone(&metrics),
                    worker,
                    crate::accept_recovery::Acceptor::default(),
                    Ok(crate::runtime_report::RuntimeReport::read_only_for_test(
                        &path,
                    )),
                )
                .await
            } else {
                server::serve_with_worker(socket.clone(), limits, Arc::clone(&metrics), worker)
                    .await
            };
            if mode == "refuse" || mode == "write-failure" {
                if mode == "write-failure" {
                    let error = result.unwrap_err();
                    assert_eq!(
                        error
                            .downcast_ref::<std::io::Error>()
                            .unwrap()
                            .raw_os_error(),
                        Some(libc::EBADF)
                    );
                    assert_eq!(
                        fs::read(socket.with_file_name("write-failure")).unwrap(),
                        b"unchanged"
                    );
                    println!("report-fixture: real write error EBADF");
                } else {
                    assert!(result.is_err());
                }
                assert!(!socket.exists());
                let snapshot = metrics.snapshot();
                assert_eq!(
                    snapshot.counter(crate::runtime_metrics::Counter::WorkerStarted),
                    1
                );
                assert_eq!(
                    snapshot.counter(crate::runtime_metrics::Counter::WorkerJoined),
                    1
                );
                for gauge in Gauge::ALL {
                    assert_eq!(snapshot.gauge(gauge).current, 0);
                }
                assert_eq!(snapshot.tracked_tasks.current, 0);
                assert_eq!(
                    snapshot.counter(crate::runtime_metrics::Counter::HandlersSpawned),
                    snapshot.counter(crate::runtime_metrics::Counter::HandlersReaped)
                );
                let reopened =
                    arc_storage::open_and_migrate(arc_core::database_path().unwrap()).unwrap();
                assert_eq!(
                    reopened
                        .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                        .unwrap(),
                    "ok"
                );
                reopened.close().unwrap();
                println!(
                    "report-fixture: refused, worker joined, gauges zero, reopen integrity ok"
                );
            } else {
                result.unwrap();
            }
        });
}

fn spawn(fixture: &Fixture, report: Option<&Path>, refuse: bool) -> OwnedChild {
    let mut command = fixture.command("lifecycle_tests::report_lifecycle_tests::report_child");
    command.env("ARC_REPORT_CHILD", if refuse { "refuse" } else { "normal" });
    if let Some(path) = report {
        command.env("ARC_RUNTIME_REPORT", path);
    } else {
        command.env_remove("ARC_RUNTIME_REPORT");
    }
    OwnedChild(command.spawn().unwrap())
}

#[test]
fn disabled_export_creates_no_report_or_runtime_stdout_record() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = spawn(&fixture, None, false);
    fixture.wait_for(&mut child, "arc-daemon ready.");
    stop(&mut child, libc::SIGTERM);
    assert!(!fixture.root.join("runtime.json").exists());
    assert!(!fixture.stdout().contains("arc-daemon runtime:"));
    assert!(!fixture.socket.exists());
}

#[test]
fn requested_export_is_open_before_readiness_and_contains_complete_schema() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let report = fixture.root.join("runtime.json");
    let mut child = spawn(&fixture, Some(&report), false);
    fixture.wait_for(&mut child, "arc-daemon ready.");
    let metadata = fs::symlink_metadata(&report).unwrap();
    assert!(metadata.is_file());
    assert_eq!(metadata.mode() & 0o7777, 0o600);
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    assert_eq!(metadata.len(), 0);
    stop(&mut child, libc::SIGINT);
    let report = fixture.report();
    assert_eq!(report["profiles"]["ipc"], "default");
    assert_eq!(
        report["limits"],
        crate::runtime_report::configuration(&RuntimeLimits::test_profile())
    );
    assert_eq!(report["supervisor"]["accept_errors"], 0);
    assert!(!fixture.stdout().contains("arc-daemon runtime:"));
    // Worst fixed-schema numeric widths still fit; this is not an RSS measurement.
    fn maximize(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Number(_) => *value = u64::MAX.into(),
            serde_json::Value::Object(map) => {
                for value in map.values_mut() {
                    maximize(value);
                }
            }
            _ => {}
        }
    }
    let mut maximum = report;
    maximize(&mut maximum);
    assert!(serde_json::to_vec(&maximum).unwrap().len() <= crate::runtime_report::MAX_REPORT_BYTES);
}

#[test]
fn invalid_exports_refuse_startup_preserve_entries_and_join_worker() {
    let _serial = support::serial();
    for mode in ["existing", "symlink", "unsafe", "parent-link"] {
        let fixture = Fixture::new();
        let target = fixture.root.join("target");
        fs::write(&target, b"unchanged sentinel").unwrap();
        let report = fixture.root.join("runtime.json");
        let requested = match mode {
            "existing" => {
                fs::write(&report, b"existing content").unwrap();
                report.clone()
            }
            "symlink" => {
                symlink(&target, &report).unwrap();
                report.clone()
            }
            "unsafe" => {
                let parent = fixture.root.join("unsafe");
                fs::create_dir(&parent).unwrap();
                fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
                parent.join("report")
            }
            "parent-link" => {
                let parent = fixture.root.join("alias");
                symlink(&fixture.root, &parent).unwrap();
                parent.join("report")
            }
            _ => unreachable!(),
        };
        let mut child = spawn(&fixture, Some(&requested), true);
        wait_exit(&mut child, true);
        let stdout = fixture.stdout();
        assert!(!stdout.contains("arc-daemon ready."), "{stdout}");
        assert!(!stdout.contains("arc-daemon runtime:"), "{stdout}");
        assert!(
            stdout.contains("worker joined, gauges zero, reopen integrity ok"),
            "{stdout}"
        );
        assert!(!fixture.socket.exists());
        assert_eq!(fs::read(&target).unwrap(), b"unchanged sentinel");
        if mode == "existing" {
            assert_eq!(fs::read(&report).unwrap(), b"existing content");
        }
        if mode == "symlink" {
            assert!(fs::symlink_metadata(report)
                .unwrap()
                .file_type()
                .is_symlink());
        }
        if mode == "unsafe" || mode == "parent-link" {
            assert!(!requested.exists());
        }
    }
}

#[test]
fn report_write_failure_preserves_cleanup_and_actual_worker_accounting() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut command = fixture.command("lifecycle_tests::report_lifecycle_tests::report_child");
    let mut child = OwnedChild(
        command
            .env("ARC_REPORT_CHILD", "write-failure")
            .env_remove("ARC_RUNTIME_REPORT")
            .spawn()
            .unwrap(),
    );
    fixture.wait_for(&mut child, "arc-daemon ready.");
    let mut stalled = fixture.connect();
    stalled.write_all(&[0]).unwrap();
    // The hello response proves the stalled peer was accepted before shutdown.
    assert_eq!(
        arc_ipc::exchange(
            &mut fixture.connect(),
            &arc_ipc::Request::new("barrier", arc_ipc::OPERATION_HELLO)
        )
        .unwrap()
        .status,
        arc_ipc::STATUS_OK
    );
    stop(&mut child, libc::SIGTERM);
    assert!(fixture.stdout().contains("real write error EBADF"));
    assert!(fixture
        .stdout()
        .contains("worker joined, gauges zero, reopen integrity ok"));
    assert!(!fixture.socket.exists());
    assert!(!fixture.root.join("runtime.json").exists());
    assert!(!fixture.stdout().contains("arc-daemon runtime:"));
}
