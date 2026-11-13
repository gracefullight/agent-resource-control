//! Both Governor release modes use the same sole-spawn gate as legacy Traffic.
use super::{checkpoint, Coordination, Gate, GateRequest};
use crate::exec::{
    governor,
    preparation::write_notice,
    signals::Signals,
    traffic::{Mode, Route},
    transport::Failure,
};
use arc_ipc::{
    client::Session,
    governor::*,
    jobs::{JobId, ProcessIdentity},
    policy::{launch::LaunchResult, ObservationAvailability},
    Request,
};
use serde_json::json;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(super) fn prepare(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    mode: &mut Mode,
    signals: &Signals,
) -> Result<Coordination, Failure> {
    let cancelled = || signals.interrupted();
    let mut held = false;
    let mut synchronized = false;
    let mut reserved_synchronized = false;
    loop {
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        let input = match mode {
            Mode::Scheduled {
                generation,
                witness,
                ..
            } => {
                witness.restore_admission().map_err(|_| Failure::Witness)?;
                GovernorLaunchRequest::Reserved {
                    request: arc_ipc::policy::launch::LaunchRequest {
                        job_id: id.clone(),
                        observer: observer.clone(),
                        permit_generation: *generation,
                    },
                }
            }
            Mode::Unreserved { .. } => GovernorLaunchRequest::Unreserved {
                job_id: id.clone(),
                observer: observer.clone(),
            },
            _ => return Err(Failure::InvalidReply),
        };
        if !reserved_synchronized {
            checkpoint::pause("reserved", &json!(input), &cancelled)?;
            reserved_synchronized = true;
        }
        let continuity = match mode {
            Mode::Scheduled { continuity, .. } | Mode::Unreserved { continuity, .. } => continuity,
            _ => unreachable!(),
        };
        let started = Instant::now();
        let mut session = match Session::connect_until(
            socket,
            started + Duration::from_millis(2000 + arc_ipc::policy::launch::LAUNCH_ACK_GRACE_MS),
            &cancelled,
            Some(continuity),
        ) {
            Ok(session) => session,
            Err(Failure::Unavailable) if !held => return Ok(Coordination::Outage),
            Err(error) => return Err(error),
        };
        let mut request = Request::new(crate::request_id(), OPERATION_LAUNCH);
        request.payload = json!(input);
        // Never retry a possibly delivered release. Only a validated HELD/RETRY
        // or acknowledged UNSPAWNED response permits another decision.
        let value = session.exchange(&request, &cancelled)?;
        let result = decode_launch_result(&value).map_err(|_| Failure::InvalidReply)?;
        // A single, validated READY still authorizes this sole local spawn
        // after reconnect. Store continuity controls metadata confirmation,
        // not execution; every claim remains non-replayed and witness-backed.
        let mut deadline = started + Duration::from_secs(2);
        match &result {
            GovernorLaunchResult::Reserved { result: reserved } => {
                let GovernorLaunchRequest::Reserved { request } = &input else {
                    return Err(Failure::InvalidReply);
                };
                match reserved.as_ref() {
                    LaunchResult::Held { .. } => {
                        if !held {
                            write_notice("QUEUED\nWaiting for memory.\nStarts automatically when memory permits; other queue rules still apply.\n");
                            held = true;
                            checkpoint::pause(
                                "held",
                                &json!({"request":input,"release":result}),
                                &cancelled,
                            )?;
                        }
                        drop(session);
                        std::thread::sleep(Duration::from_millis(40));
                        continue;
                    }
                    LaunchResult::Ready {
                        permit_generation,
                        policy,
                    } if *permit_generation == request.permit_generation => {
                        if policy.observation.availability == ObservationAvailability::Available {
                            let remaining = policy
                                .observation
                                .stale_after_ms
                                .zip(policy.observation.age_ms)
                                .and_then(|(limit, age)| limit.checked_sub(age))
                                .ok_or(Failure::InvalidReply)?;
                            deadline = deadline.min(started + Duration::from_millis(remaining));
                        }
                    }
                    LaunchResult::Reused { permit_generation }
                        if *permit_generation == request.permit_generation =>
                    {
                        return Ok(Coordination::Legacy)
                    }
                    LaunchResult::Cancelled {} => return Err(Failure::Cancelled),
                    _ => return Err(Failure::InvalidReply),
                }
            }
            GovernorLaunchResult::Unreserved {
                release_id,
                valid_for_ms,
                ..
            } => {
                deadline = deadline.min(started + Duration::from_millis(*valid_for_ms));
                mark_unreserved(mode, release_id.clone(), held)?;
            }
            GovernorLaunchResult::Retry {} => {
                drop(session);
                governor::repoll(socket, id, observer, mode, &cancelled)?;
                continue;
            }
            GovernorLaunchResult::Unconfirmed {} => return Err(Failure::Uncertain),
            GovernorLaunchResult::Cancelled {} => return Err(Failure::Cancelled),
            GovernorLaunchResult::NotFound {} => return Err(Failure::Rejected),
        }
        if !synchronized {
            checkpoint::pause(
                "ready",
                &json!({"request":input,"release":result}),
                &cancelled,
            )?;
            synchronized = true;
        }
        let mut gate = Gate {
            session,
            request: GateRequest::Governor {
                request: input,
                result,
            },
            waited: held,
            deadline,
        };
        if Instant::now() >= deadline {
            if !gate.acknowledge(None) {
                return Err(Failure::Uncertain);
            }
            drop(gate);
            governor::repoll(socket, id, observer, mode, &cancelled)?;
            continue;
        }
        return Ok(Coordination::Fenced(gate));
    }
}
fn mark_unreserved(mode: &mut Mode, release: String, held: bool) -> Result<(), Failure> {
    if !matches!(mode, Mode::Unreserved { .. }) {
        let old = std::mem::replace(
            mode,
            Mode::Legacy(crate::exec::preparation::Tracking::Uncertain),
        );
        let Mode::Scheduled {
            witness,
            continuity,
            waited,
            route: Route::Governor,
            ..
        } = old
        else {
            unreachable!()
        };
        *mode = Mode::Unreserved {
            witness,
            continuity,
            waited,
            release_id: None,
        };
    }
    let Mode::Unreserved {
        witness,
        release_id,
        waited,
        ..
    } = mode
    else {
        unreachable!()
    };
    *release_id = Some(release.clone());
    *waited |= held;
    witness.bind_release(&release).map_err(|_| Failure::Witness)
}
