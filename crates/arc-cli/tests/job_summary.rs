//! Synthetic authenticated peers exercise the production CLI JSON read. These
//! are deterministic protocol fixtures, not persisted native acceptance.
use arc_core::job_meter::*;
use arc_ipc::{Request, Response};
use std::{
    fs, io,
    os::unix::{
        fs::{DirBuilderExt, PermissionsExt},
        net::UnixListener,
    },
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

const JOB: &str = "0123456789abcdef0123456789abcdef";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = arc_core::runtime_root().unwrap().join(format!(
            "js-{:x}-{:x}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self(root)
    }
    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_arcctl"))
            .args(args)
            .env(arc_core::ARC_SOCKET_PATH, self.0.join("ipc"))
            .output()
            .unwrap()
    }
    fn peer(&self, case: u8) -> Output {
        let listener = UnixListener::bind(self.0.join("ipc")).unwrap();
        fs::set_permissions(self.0.join("ipc"), fs::Permissions::from_mode(0o600)).unwrap();
        listener.set_nonblocking(true).unwrap();
        thread::scope(|scope| {
            let peer = scope.spawn(|| {
                let end = Instant::now() + Duration::from_secs(4);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < end, "summary read never arrived");
                            thread::sleep(Duration::from_millis(1));
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let request: Request = arc_ipc::read_message(&mut stream).unwrap();
                assert_eq!(request.operation, "meter.summary");
                assert_eq!(request.protocol_version, 1);
                assert_eq!(request.payload, serde_json::json!({"job_id": JOB}));
                if case == 13 {
                    // Close only after the CLI's absolute read budget expires.
                    thread::sleep(Duration::from_millis(2150));
                    return;
                }
                let mut report: JobMeterReport = serde_json::from_str(include_str!(
                    "../../arc-core/tests/fixtures/job-meter-report.json"
                ))
                .unwrap();
                report.job_id = JOB.into();
                report.accounting_relation = AccountingRelation::CanonicalRoot {
                    root_job_id: JOB.into(),
                };
                let mut summary = JobMeterSummary {
                    schema_version: 1,
                    job_id: JOB.into(),
                    accounting: SummaryAccounting::Recorded {
                        report: Box::new(report),
                    },
                    queue_wait: QueueWait::lifecycle_only(),
                };
                match case {
                    1 => {
                        summary.accounting = SummaryAccounting::Unknown {
                            reason: UnknownReason::Legacy,
                        };
                        summary.queue_wait = QueueWait::unknown();
                    }
                    2 => {
                        summary.accounting = SummaryAccounting::Unknown {
                            reason: UnknownReason::Interrupted,
                        };
                        summary.queue_wait = QueueWait::unknown();
                    }
                    3 => {
                        summary.accounting = SummaryAccounting::Expired {};
                        summary.queue_wait = QueueWait::unknown();
                    }
                    4 => {
                        summary.accounting = SummaryAccounting::Missing {};
                        summary.queue_wait = QueueWait::unknown();
                    }
                    _ => (),
                }
                assert!(summary.is_valid());
                let mut value = serde_json::to_value(summary).unwrap();
                let mut response = Response::ok(&request.request_id, value.clone());
                match case {
                    5 => {
                        response = Response::error(
                            &request.request_id,
                            arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                        )
                    }
                    6 => {
                        response = Response::error(
                            &request.request_id,
                            arc_ipc::meter::ERROR_METER_UNAVAILABLE,
                        )
                    }
                    7 => {
                        value["job_id"] = serde_json::json!("f".repeat(32));
                        response.result = Some(value);
                    }
                    8 => {
                        value["accounting"]["report"]["cpu_core_seconds"]["total"] =
                            serde_json::json!(-1);
                        response.result = Some(value);
                    }
                    9 => {
                        value["peer-secret-canary"] = true.into();
                        response.result = Some(value);
                    }
                    10 => {
                        response =
                            Response::error(&request.request_id, arc_ipc::ERROR_INVALID_REQUEST)
                    }
                    11 => response.protocol_version = 99,
                    12 => response.request_id = "peer-secret-canary".into(),
                    _ => (),
                }
                if let Some(error) = &mut response.error {
                    error.message = Some("peer-secret-canary".into());
                }
                arc_ipc::write_response(&mut stream, &response).unwrap();
            });
            let output = self.cli(&["meter", "summary", "--job", JOB, "--json"]);
            peer.join().unwrap();
            output
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn summary_json_correlates_the_job_and_preserves_queue_zero_and_unknown_states() {
    for case in 0..=4 {
        let output = Fixture::new().peer(case);
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert_eq!(output.stdout.iter().filter(|b| **b == b'\n').count(), 1);
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let summary = arc_ipc::meter::decode_summary(&value, JOB).unwrap();
        assert_eq!(summary.job_id, JOB);
        if case == 0 {
            assert_eq!(summary.queue_wait.seconds, Some(0.0));
            assert!(matches!(
                summary.accounting,
                SummaryAccounting::Recorded { .. }
            ));
            assert!(value["accounting"]["report"]["cpu_core_seconds"]["total"]
                .as_f64()
                .is_some());
        } else {
            assert_eq!(summary.queue_wait.seconds, None);
            assert!(value["queue_wait"]["seconds"].is_null());
            assert!(value["accounting"].get("report").is_none());
        }
    }
}
#[test]
fn unsupported_unavailable_bad_correlation_and_untrusted_replies_are_redacted() {
    for case in 5..=12 {
        let output = Fixture::new().peer(case);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(
            error.starts_with(match case {
                5 => "UNSUPPORTED_CAPABILITY",
                6 => "METER_UNAVAILABLE",
                11 => "PROTOCOL_INCOMPATIBLE",
                _ => "IPC_ERROR",
            }),
            "{case}: {error}"
        );
        assert!(!error.contains("peer-secret-canary"));
        assert!(!error.contains("cpu_core_seconds"));
    }
}
#[test]
fn invalid_job_arguments_never_connect_and_help_explains_lookup_and_units() {
    let f = Fixture::new();
    for args in [
        vec!["meter", "summary", "--json"],
        vec!["meter", "summary", "--job", JOB],
        vec!["meter", "summary", "--job", "peer-secret-canary", "--json"],
        vec!["meter", "summary", "--job", JOB, "--json", "--pid", "1"],
        vec!["meter", "summary", "--job", JOB, "--json", "/secret"],
    ] {
        let output = f.cli(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("peer-secret-canary"));
    }
    for flag in ["-h", "--help"] {
        let output = f.cli(&["meter", "summary", flag]);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let help = String::from_utf8(output.stdout).unwrap();
        for text in [
            "--job",
            "--json",
            "arcctl job status",
            "CPU core-seconds",
            "Unknown",
            "Partial",
        ] {
            assert!(help.contains(text), "missing {text}: {help}");
        }
    }
    let help = f.cli(&["meter", "--help"]);
    assert!(String::from_utf8(help.stdout).unwrap().contains("summary"));
}
#[test]
fn a_stalled_summary_is_bounded_and_returns_no_fabricated_measurement() {
    let start = Instant::now();
    let output = Fixture::new().peer(13);
    assert!(start.elapsed() < Duration::from_millis(2800));
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("METER_UNAVAILABLE"));
}
