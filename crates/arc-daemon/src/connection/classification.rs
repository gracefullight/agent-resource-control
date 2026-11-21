use arc_ipc::{Request, Response};

// The clock is supplied at the dispatch boundary, so tests can exercise the
// real daemon route without depending on how long an IPC request takes.
pub(super) fn classify_request(request: &Request, now_ms: u64) -> Option<Response> {
    match request.operation.as_str() {
        arc_ipc::locality::OPERATION_CLASSIFY | arc_ipc::compute_location::OPERATION_CLASSIFY => {
            Some(arc_ipc::locality::classify_response_at(request, now_ms))
        }
        arc_ipc::OPERATION_JOB_CLASSIFY => Some(arc_ipc::dispatch(request, || {
            unreachable!("classification never uses SQLite")
        })),
        _ => None,
    }
}

#[cfg(test)]
mod locality_clock_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn daemon_dispatch_uses_injected_clock_for_fresh_stale_and_future_context() {
        for command in [
            json!({"kind":"SHELL","command":"gh workflow run build.yml","shell_kind":"sh"}),
            json!({"kind":"ARGV","argv":["gh","workflow","run","build.yml"]}),
        ] {
            let mut request = Request::new("clocked", arc_ipc::locality::OPERATION_CLASSIFY);
            request.payload = json!({"command":command,"context":{
                "captured_at_ms":10000,
                "environment":{"GH_HOST":"github.com"},
                "facts":[{"kind":"CI_HOST","selector":"gh","endpoints":["https://github.com"]}]
            }});
            for (now, expected) in [
                (10_000, "REMOTE_CI"),
                (15_000, "REMOTE_CI"),
                (15_001, "UNKNOWN"),
                (9_999, "UNKNOWN"),
            ] {
                let response = classify_request(&request, now).unwrap();
                assert_eq!(response.protocol_version, 1);
                assert_eq!(response.status, "ok");
                assert_eq!(response.result.unwrap()["classification"], expected);
            }
        }
    }
}
