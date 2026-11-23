//! Optional negotiated workspace detail; ordinary classifications keep their
//! four-field legacy projection and unresolved graphs never enter this path.
use arc_detector::scripts::MemberClassification;
use arc_ipc::{
    jobs::JobId,
    workspace_compound::{Compound, CompoundRequest, Member, OPERATION},
    Classification,
};
use std::{
    cell::RefCell,
    path::Path,
    time::{Duration, Instant},
};

thread_local! {
    static CURRENT: RefCell<Option<Compound>> = const { RefCell::new(None) };
}

pub(crate) fn clear() {
    CURRENT.with(|current| *current.borrow_mut() = None);
}

pub(crate) fn candidate(
    members: &[MemberClassification],
    projection: Classification,
) -> Option<Compound> {
    let members = members
        .iter()
        .map(|m| {
            Some(Member {
                member: m.member.to_str()?.to_owned(),
                script: m.script.clone(),
                classification: m.classification,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let compound = Compound::local(members)?;
    (compound.admission == projection).then_some(compound)
}

pub(crate) fn capture(members: &[MemberClassification], projection: Classification) -> bool {
    let candidate = candidate(members, projection);
    let found = candidate.is_some();
    CURRENT.with(|current| *current.borrow_mut() = candidate);
    found
}

/// The response must be exactly the result locally derived from the validated
/// project analysis. An old daemon simply returns unsupported.
pub(crate) fn negotiate(
    socket: &Path,
    candidate: &Compound,
    job_id: Option<JobId>,
) -> Option<Compound> {
    negotiate_until(
        socket,
        candidate,
        job_id,
        Instant::now() + Duration::from_secs(2),
        &|| false,
    )
}

fn negotiate_until(
    socket: &Path,
    candidate: &Compound,
    job_id: Option<JobId>,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Option<Compound> {
    let mut session =
        arc_ipc::client::Session::connect_until(socket, deadline, cancelled, None).ok()?;
    let mut probe = arc_ipc::Request::new(crate::request_id(), OPERATION);
    probe.payload = serde_json::to_value(CompoundRequest::Probe).ok()?;
    if session.exchange(&probe, cancelled).ok()? != serde_json::json!({"capability": OPERATION}) {
        return None;
    }
    let mut request = arc_ipc::Request::new(crate::request_id(), OPERATION);
    request.payload = serde_json::to_value(CompoundRequest::Analyze {
        members: candidate.members.clone(),
        job_id,
    })
    .ok()?;
    let result = session.exchange(&request, cancelled).ok()?;
    let result: Compound = serde_json::from_value(result.get("compound")?.clone()).ok()?;
    (result == *candidate).then_some(result)
}

/// Publish a job-scoped negotiated result before its existing registration.
/// A missing or earlier daemon leaves the original conservative projection.
pub(super) fn publish(
    socket: &Path,
    id: &JobId,
    projection: Classification,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Classification {
    let candidate = CURRENT.with(|current| current.borrow_mut().take());
    candidate
        .filter(|candidate| candidate.admission == projection)
        .and_then(|candidate| {
            negotiate_until(
                socket,
                &candidate,
                Some(id.clone()),
                deadline.min(Instant::now() + Duration::from_secs(2)),
                cancelled,
            )
        })
        .map_or(projection, |result| result.admission)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_ipc::{JobClass, Locality, Reason, Uncertainty};
    use std::{
        fs,
        os::unix::{fs::PermissionsExt, net::UnixListener},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread,
        time::Duration,
    };

    #[test]
    fn unsupported_peer_keeps_the_legacy_projection() {
        let name = arc_core::process_identity::random_job_id().unwrap();
        let socket = arc_core::runtime_root()
            .unwrap()
            .join(format!("arc-wc-{}.sock", &name[..12]));
        let listener = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let done = Arc::new(AtomicBool::new(false));
        let peer_done = done.clone();
        let peer = thread::spawn(move || {
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if peer_done.load(Ordering::Acquire) {
                            return false;
                        }
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) => panic!("accept: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(60)))
                .unwrap();
            let request: arc_ipc::Request =
                serde_json::from_slice(&arc_ipc::read_frame(&mut stream).unwrap()).unwrap();
            assert_eq!(request.operation, OPERATION);
            assert_eq!(request.payload, serde_json::json!({"action": "probe"}));
            arc_ipc::write_message(
                &mut stream,
                &arc_ipc::Response::error(
                    &request.request_id,
                    arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                ),
            )
            .unwrap();
            true
        });
        let leaf = |member: &str, class, reason| Member {
            member: member.into(),
            script: "ci".into(),
            classification: Classification {
                classification: class,
                locality: Locality::HostLocal,
                reason,
                uncertainty: Uncertainty::None,
            },
        };
        let candidate = Compound::local(vec![
            leaf("a", JobClass::Test, Reason::BunTest),
            leaf("b", JobClass::Build, Reason::BunScriptBuild),
        ])
        .unwrap();
        let result = negotiate(&socket, &candidate, None);
        done.store(true, Ordering::Release);
        let exchanged = peer.join().unwrap();
        fs::remove_file(socket).unwrap();
        assert!(exchanged, "the old peer received the negotiated operation");
        assert!(result.is_none());
        assert_eq!(candidate.admission.classification, JobClass::Unknown);
        assert_eq!(candidate.admission.uncertainty, Uncertainty::Observe);
    }
}
