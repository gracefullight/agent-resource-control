use super::*;
use serde_json::json;

fn fixture() -> JobMeterReport {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../arc-core/tests/fixtures/job-meter-report.json"
    )))
    .unwrap()
}
fn summary() -> JobMeterSummary {
    JobMeterSummary {
        schema_version: 1,
        job_id: fixture().job_id,
        accounting: SummaryAccounting::Recorded {
            report: Box::new(fixture()),
        },
        queue_wait: QueueWait::lifecycle_only(),
    }
}
fn receipt() -> ReportReceipt {
    ReportReceipt {
        schema_version: 1,
        job_id: fixture().job_id,
        report_id: fixture().report_id,
        disposition: ReportDisposition::Accepted,
    }
}

#[test]
fn report_and_summary_are_additive_correlated_protocol_one_operations() {
    let report = fixture();
    let request = report_request("r1", &report).unwrap();
    assert_eq!(request.protocol_version, 1);
    assert_eq!(request.operation, "meter.report");
    assert_eq!(decode_report(&request.payload), Ok(report));
    assert!(matches!(
        JobMeterRequest::decode(&request.operation, &request.payload),
        Some(Ok(JobMeterRequest::Report(_)))
    ));
    let response = report_response(&request, &receipt());
    assert_eq!(interpret_report(&request, &response), Ok(receipt()));
    let mut retry = receipt();
    retry.disposition = ReportDisposition::Unchanged;
    assert_eq!(
        interpret_report(&request, &report_response(&request, &retry)),
        Ok(retry)
    );
    let query = summary_request("s1", &summary().job_id).unwrap();
    assert_eq!(query.operation, "meter.summary");
    assert!(matches!(
        JobMeterRequest::decode(&query.operation, &query.payload),
        Some(Ok(JobMeterRequest::Summary(_)))
    ));
    assert_eq!(
        interpret_summary(&query, &summary_response(&query, &summary())),
        Ok(summary())
    );
    assert!(JobMeterRequest::decode("job.complete", &request.payload).is_none());
    assert!(JobMeterRequest::decode("meter.snapshot", &Value::Null).is_none());
}

#[test]
fn invalid_payloads_are_bounded_and_never_extend_completion_or_snapshot() {
    let value = serde_json::to_value(fixture()).unwrap();
    let invalid = [
        Value::Null,
        json!([]),
        json!({"job_id":"bad"}),
        {
            let mut v = value.clone();
            v["argv"] = json!(["SECRET"]);
            v
        },
        {
            let mut v = value.clone();
            v["cpu_core_seconds"] = json!([1.25, 0.75, 2.0]);
            v
        },
        {
            let mut v = value.clone();
            v["run_elapsed"] = json!([3.0, "monotonic", true]);
            v
        },
        {
            let mut v = value;
            v["report_id"] = json!("x".repeat(MAX_JOB_METER_BYTES));
            v
        },
    ];
    for input in invalid {
        assert_eq!(decode_report(&input), Err(ERROR_INVALID_REQUEST));
    }
    for query in [
        Value::Null,
        json!([fixture().job_id]),
        json!({"job_id":fixture().job_id,"pid":101}),
    ] {
        assert!(decode_summary_request(&query).is_err());
    }
    assert!(summary_request("q", "not-a-job-id").is_err());
    let mut invalid_report = fixture();
    invalid_report.cpu_core_seconds.total = Some(-1.0);
    assert!(report_request("r", &invalid_report).is_err());
    let complete = json!({"job_id":fixture().job_id,"observer":fixture().observer,"outcome":fixture().outcome});
    assert!(crate::jobs::decode_complete(&complete).is_ok());
    let mut extended = complete;
    extended["cpu_core_seconds"] = json!(1.0);
    assert!(crate::jobs::decode_complete(&extended).is_err());
    assert!(super::super::decode_request(&Value::Null).is_ok());
    assert!(super::super::decode_request(&json!({"job_id":fixture().job_id})).is_err());
    let query = summary_request("old-daemon", &fixture().job_id).unwrap();
    let old = crate::dispatch(&query, || unreachable!());
    assert_eq!(
        interpret_summary(&query, &old),
        Err(JobMeterError::Unsupported)
    );
    assert!(!format!("{query:?}").contains(&fixture().job_id));
}

#[test]
fn typed_nonfinite_values_are_rejected_before_lossy_json_serialization() {
    let mut report = fixture();
    report.completeness = Completeness::Partial;
    report.missing_coverage = vec![MissingCoverageReason::NativeUsageUnavailable];
    report.cpu_core_seconds = CpuCoreSeconds {
        user: Some(f64::NAN),
        system: None,
        total: None,
    };
    assert!(!report.is_valid());
    assert!(report_request("invalid-number", &report).is_err());
    let query = summary_request("invalid-summary", &report.job_id).unwrap();
    let mut invalid = summary();
    invalid.accounting = SummaryAccounting::Recorded {
        report: Box::new(report),
    };
    assert_eq!(summary_response(&query, &invalid).status, STATUS_ERROR);
}

#[test]
fn reject_cross_job_replies_mismatched_receipts_and_protocols() {
    let query = summary_request("s", &fixture().job_id).unwrap();
    let mut bad_summary = summary();
    bad_summary.job_id = "3".repeat(32);
    assert_eq!(summary_response(&query, &bad_summary).status, STATUS_ERROR);
    let mut response = summary_response(&query, &summary());
    response.request_id = "stale".into();
    assert_eq!(
        interpret_summary(&query, &response),
        Err(JobMeterError::InvalidReply)
    );
    response.request_id = "s".into();
    response.protocol_version = 2;
    assert_eq!(
        interpret_summary(&query, &response),
        Err(JobMeterError::Protocol)
    );
    let request = report_request("r", &fixture()).unwrap();
    for field in ["report_id", "job_id", "schema_version"] {
        let mut invalid = receipt();
        match field {
            "report_id" => invalid.report_id = "3".repeat(32),
            "job_id" => invalid.job_id = "3".repeat(32),
            _ => invalid.schema_version = 2,
        }
        assert_eq!(report_response(&request, &invalid).status, STATUS_ERROR);
        let response = Response::ok("r", serde_json::to_value(invalid).unwrap());
        assert_eq!(
            interpret_report(&request, &response),
            Err(JobMeterError::InvalidReply)
        );
    }
    let mut version = query.clone();
    version.protocol_version = 2;
    assert_eq!(
        summary_response(&version, &summary()).error.unwrap().code,
        crate::ERROR_PROTOCOL_INCOMPATIBLE
    );
    let mut wrong_op = request;
    wrong_op.operation = "job.complete".into();
    assert_eq!(report_response(&wrong_op, &receipt()).status, STATUS_ERROR);
}

#[test]
fn errors_distinguish_conflict_identity_terminal_and_unavailable() {
    let request = report_request("r", &fixture()).unwrap();
    for (code, expected) in [
        (
            crate::ERROR_UNSUPPORTED_CAPABILITY,
            JobMeterError::Unsupported,
        ),
        (
            super::super::ERROR_METER_UNAVAILABLE,
            JobMeterError::Unavailable,
        ),
        (ERROR_METER_REPORT_CONFLICT, JobMeterError::Conflict),
        (
            ERROR_METER_IDENTITY_MISMATCH,
            JobMeterError::IdentityMismatch,
        ),
        (ERROR_METER_NOT_TERMINAL, JobMeterError::NotTerminal),
        (crate::ERROR_PROTOCOL_INCOMPATIBLE, JobMeterError::Protocol),
        ("secret peer diagnostic", JobMeterError::InvalidReply),
    ] {
        assert_eq!(
            interpret_report(&request, &Response::error("r", code)),
            Err(expected)
        );
    }
    let mut malformed = report_response(&request, &receipt());
    malformed.error = Some(crate::ErrorBody {
        code: "secret".into(),
        message: None,
    });
    assert_eq!(
        interpret_report(&request, &malformed),
        Err(JobMeterError::InvalidReply)
    );
}

#[test]
fn terminal_outcome_binding_includes_signal_and_core_dump_state() {
    let mut report = fixture();
    assert!(matches_terminal_outcome(
        &report,
        &crate::jobs::Outcome::Exited { code: 0 }
    ));
    assert!(!matches_terminal_outcome(
        &report,
        &crate::jobs::Outcome::Exited { code: 17 }
    ));
    report.outcome = TerminalOutcome::Signaled {
        signal: 15,
        core_dumped: false,
    };
    assert!(matches_terminal_outcome(
        &report,
        &crate::jobs::Outcome::Signaled {
            signal: 15,
            core_dumped: false
        }
    ));
    assert!(!matches_terminal_outcome(
        &report,
        &crate::jobs::Outcome::Signaled {
            signal: 15,
            core_dumped: true
        }
    ));
    assert!(!matches_terminal_outcome(
        &report,
        &crate::jobs::Outcome::SpawnFailed {
            reason: crate::jobs::SpawnReason::Other,
            os_code: None
        }
    ));
}

#[cfg(unix)]
#[path = "transport_tests.rs"]
mod transport_tests;
