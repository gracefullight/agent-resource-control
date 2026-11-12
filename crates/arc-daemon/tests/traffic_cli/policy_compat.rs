//! Legacy fixtures explicitly reject only the additive policy display probe.
pub(super) fn unsupported_probe(request: &arc_ipc::Request) -> Option<arc_ipc::Response> {
    if request.operation != arc_ipc::policy::OPERATION_POLICY_CAPABILITIES {
        return None;
    }
    assert!(request.payload.is_null());
    Some(arc_ipc::Response::error(
        &request.request_id,
        arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
    ))
}
