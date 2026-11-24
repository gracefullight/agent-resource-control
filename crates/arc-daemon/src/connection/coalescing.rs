use arc_ipc::{coalescing, meter::ERROR_METER_UNAVAILABLE, Request, Response};
use arc_meter::SnapshotReader;

pub(super) fn response(request: &Request, reader: Option<&SnapshotReader>) -> Response {
    let poll = match coalescing::decode(&request.payload) {
        Ok(poll) => poll,
        Err(code) => return Response::error(&request.request_id, code),
    };
    match reader.and_then(SnapshotReader::snapshot) {
        Some(snapshot) => Response::ok(
            &request.request_id,
            serde_json::to_value(coalescing::latest(&poll, snapshot))
                .expect("serializable latest event"),
        ),
        None => Response::error(&request.request_id, ERROR_METER_UNAVAILABLE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_cursor_and_missing_sampler_are_distinct() {
        let mut request = coalescing::request("events", None);
        assert_eq!(
            response(&request, None).error.unwrap().code,
            ERROR_METER_UNAVAILABLE
        );
        request.payload = serde_json::json!({"after": null, "extra": true});
        assert_eq!(
            response(&request, None).error.unwrap().code,
            arc_ipc::ERROR_INVALID_REQUEST
        );
    }
}
