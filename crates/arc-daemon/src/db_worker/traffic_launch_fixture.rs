//! Keep existing lost-reply fixtures on the production fenced launch route.
pub(super) fn dispatch(
    runtime: &tokio::runtime::Handle,
    client: &super::DbClient,
    stream: &std::os::unix::net::UnixStream,
    request: &arc_ipc::Request,
) -> bool {
    if !matches!(
        request.operation.as_str(),
        arc_ipc::policy::launch::OPERATION_TRAFFIC_LAUNCH
            | arc_ipc::policy::OPERATION_POLICY_CAPABILITIES
    ) {
        return false;
    }
    let stream = stream.try_clone().unwrap();
    stream.set_nonblocking(true).unwrap();
    let client = client.clone();
    runtime.block_on(async move {
        let stream = tokio::net::UnixStream::from_std(stream).unwrap();
        let limits = crate::limits::RuntimeLimits::default();
        let metrics = std::sync::Arc::new(crate::runtime_metrics::RuntimeMetrics::new(&limits));
        let handler: crate::connection::policy_dispatch::Handler =
            std::sync::Arc::new(move |input, peer, deadline| {
                Box::pin(super::super::policy::handle(
                    client.clone(),
                    input,
                    peer,
                    deadline,
                ))
            });
        crate::connection::policy_dispatch::dispatch(
            &stream,
            request,
            &limits,
            Some(&handler),
            &metrics,
        )
        .await
    })
}
