//! The policy-capable adapter retains the shared transport admission.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_with_policy<D, F, J, G, T, H>(
    stream: UnixStream,
    limits: RuntimeLimits,
    requests: Requests,
    metrics: Arc<RuntimeMetrics>,
    doctor: D,
    jobs: J,
    traffic: (
        T,
        Option<arc_meter::SnapshotReader>,
        Option<crate::job_meter_dispatch::Handler>,
    ),
    policy: Option<policy_dispatch::Handler>,
) where
    D: Fn(std::time::Instant) -> F,
    F: Future<Output = DoctorOutcome>,
    J: Fn(arc_ipc::jobs::LifecycleRequest, std::time::Instant) -> G,
    G: Future<Output = Result<serde_json::Value, &'static str>>,
    T: Fn(arc_ipc::traffic::JobRequest, u32, std::time::Instant) -> H,
    H: Future<Output = Result<serde_json::Value, &'static str>>,
{
    handle_inner(
        stream,
        limits,
        requests,
        metrics,
        doctor,
        jobs,
        TrafficHandler {
            enabled: true,
            handle: traffic.0,
            meter: traffic.1,
            meter_jobs: traffic.2,
            policy,
        },
    )
    .await;
}
