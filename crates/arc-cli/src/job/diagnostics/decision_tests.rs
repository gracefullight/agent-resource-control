//! The optional raw-reason read against a scripted peer on one connection.
use super::*;
use arc_ipc::{
    read_message, traffic::diagnostics as wire, write_response, Response,
    ERROR_UNSUPPORTED_CAPABILITY,
};
use std::{os::unix::net::UnixListener, path::PathBuf, thread, time::Duration};

fn stored(kind: &str, reason: &str) -> DiagnosticResult {
    use arc_ipc::policy::{
        evaluator::{evaluate, MissingObservation},
        ObservationProvider, RecordedDecision, POLICY_VERSION,
    };
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../apps/desktop/tests/fixtures/traffic-scripts-wire.json"
    ))
    .unwrap();
    // A running job holding permit generation 1.
    let mut result: DiagnosticResult =
        serde_json::from_value(fixture["results"][2].clone()).unwrap();
    let mut effective = None;
    MissingObservation.with_observation(&mut |input| effective = Some(evaluate(input)));
    let admission = result.admission.as_mut().unwrap();
    admission.last_decision = Some(RecordedDecision {
        policy_version: POLICY_VERSION.into(),
        kind: serde_json::from_value(json!(kind)).unwrap(),
        reason: serde_json::from_value(json!(reason)).unwrap(),
        decided_at_ms: result.recorded_at_ms,
        permit_generation: (kind == "admit").then_some(1),
        observation: effective.unwrap().observation,
    });
    assert!(result.is_valid(), "{result:?}");
    result
}

/// Serve the diagnostics query and then `second` on the same connection.
fn inspect(
    result: &DiagnosticResult,
    second: Option<fn(&Request, &JobId) -> Response>,
) -> (DiagnosticResult, Option<FairnessReason>, Vec<String>) {
    let directory: PathBuf = std::env::temp_dir().join(format!(
        "arc-decision-{}-{:?}",
        std::process::id(),
        thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let socket = directory.join("arc.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let (served, job) = (result.clone(), result.job_id.clone());
    let server = thread::spawn(move || {
        let mut operations = Vec::new();
        // One capability probe, then the query connection.
        for connection in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(60)))
                .unwrap();
            let request: Request = read_message(&mut stream).unwrap();
            operations.push(request.operation.clone());
            let reply = if connection == 0 {
                json!({"capability":arc_ipc::traffic::native::DIAGNOSTICS_CAPABILITY})
            } else {
                json!(served)
            };
            write_response(&mut stream, &Response::ok(&request.request_id, reply)).unwrap();
            if connection == 1 {
                if let Some(second) = second {
                    if let Ok(request) = read_message::<_, Request>(&mut stream) {
                        operations.push(request.operation.clone());
                        write_response(&mut stream, &second(&request, &job)).unwrap();
                    }
                }
            }
        }
        // No further connection is ever opened for the optional read.
        listener.set_nonblocking(true).unwrap();
        assert!(listener.accept().is_err());
        operations
    });
    let (read, fair, _) = super::super::inspect(&socket, &result.job_id).unwrap();
    let operations = server.join().unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    (read, fair, operations)
}

fn reason(tag: Value) -> fn(&Request, &JobId) -> Response {
    // Function pointers carry no state: one per scripted reply.
    match tag.as_str() {
        Some("project_fair_turn") => |request, job| {
            assert_eq!(request.payload, json!({"job_id":job}));
            let result = json!({"schema_version":1,"job_id":job,"reason":"project_fair_turn"});
            Response::ok(&request.request_id, result)
        },
        Some("admitted") => |request, job| {
            let result = json!({"schema_version":1,"job_id":job,"reason":"admitted"});
            Response::ok(&request.request_id, result)
        },
        Some("other-job") => |request, _| {
            let result =
                json!({"schema_version":1,"job_id":"f".repeat(32),"reason":"project_fair_turn"});
            Response::ok(&request.request_id, result)
        },
        Some("unsupported") => {
            |request, _| Response::error(&request.request_id, ERROR_UNSUPPORTED_CAPABILITY)
        }
        _ => |request, job| {
            let result = json!({"schema_version":1,"job_id":job,"reason":null});
            Response::ok(&request.request_id, result)
        },
    }
}

#[test]
fn a_fair_turn_is_read_on_the_same_connection_and_rendered_in_both_forms() {
    let admitted = stored("admit", "admitted");
    let (read, fair, operations) = inspect(&admitted, Some(reason(json!("project_fair_turn"))));
    assert_eq!(read, admitted);
    assert_eq!(fair, Some(FairnessReason::ProjectFairTurn));
    assert_eq!(operations.last().unwrap(), OPERATION_DECISION);
    assert_eq!(
        operations[1],
        arc_ipc::traffic::native::OPERATION_DIAGNOSTICS_JOB
    );
    assert_eq!(
        line(&read, fair),
        "Last recorded decision: project_fair_turn\n"
    );
    let document = document(&read, fair);
    assert_eq!(document["last_recorded_reason"], "project_fair_turn");
    // The v1 object inside is unchanged and still names only `admitted`.
    let mut without = document.clone();
    without
        .as_object_mut()
        .unwrap()
        .remove("last_recorded_reason");
    assert_eq!(without, json!(read));
    assert!(wire::decode_result(&without, &read.job_id).is_ok());
    assert_eq!(without["admission"]["last_decision"]["reason"], "admitted");
}

#[test]
fn every_other_answer_leaves_the_projected_decision_and_the_same_output() {
    let admitted = stored("admit", "admitted");
    for second in [
        Some(reason(json!("admitted"))),
        Some(reason(json!("other-job"))),
        Some(reason(json!("unsupported"))),
        Some(reason(json!("none"))),
        // The peer closes the connection after the diagnostics reply.
        None,
    ] {
        let (read, fair, _) = inspect(&admitted, second);
        assert_eq!(fair, None);
        assert_eq!(line(&read, fair), "Last recorded decision: admitted\n");
        assert_eq!(document(&read, fair), json!(read));
    }
}

#[test]
fn a_decision_that_cannot_hide_a_fair_turn_is_never_asked_about() {
    let waiting = stored("wait", "class_window");
    let (read, fair, operations) = inspect(&waiting, None);
    assert_eq!(fair, None);
    assert_eq!(operations.len(), 2, "{operations:?}");
    assert_eq!(line(&read, fair), "Last recorded decision: class_window\n");
    let mut absent = waiting.clone();
    absent.admission.as_mut().unwrap().last_decision = None;
    assert_eq!(line(&absent, None), "");
    absent.admission = None;
    assert_eq!(line(&absent, None), "");
    assert_eq!(document(&absent, None), json!(absent));
}
