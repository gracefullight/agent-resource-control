//! Bounded admission retries using the caller's authenticated route.
use super::*;

pub(super) enum Admission {
    Unconfirmed,
    Reserved {
        generation: u64,
        context: LineageContext,
        waited: bool,
    },
    Unreserved {
        waited: bool,
    },
}
#[derive(Clone, Copy)]
pub(super) struct AdmissionPeer<'a> {
    pub(super) socket: &'a Path,
    pub(super) id: &'a JobId,
    pub(super) observer: &'a ProcessIdentity,
    pub(super) continuity: &'a Continuity,
    pub(super) route: Route,
}
pub(super) fn await_admission(
    peer: AdmissionPeer<'_>,
    initial: &JobRequest,
    inherited: bool,
    claim: Option<&LineageContext>,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<Admission, Failure> {
    #[cfg(not(test))]
    let timing = Timing {
        now: &Instant::now,
        sleep: &std::thread::sleep,
    };
    #[cfg(test)]
    let timing = Timing {
        now: &test_clock::now,
        sleep: &test_clock::sleep,
    };
    let transport = |request: &GovernorJobRequest, deadline, committed: &mut bool| {
        call_routed(
            peer.socket,
            request,
            deadline,
            cancelled,
            peer.continuity,
            peer.route,
            committed,
        )
    };
    await_admission_with(
        peer,
        initial,
        inherited,
        claim,
        deadline,
        cancelled,
        Runtime { timing, transport },
    )
}

/// Thread-local virtual time keeps the full CLI outage regression deterministic.
/// Production always uses the actual monotonic clock and sleeping implementation.
#[cfg(test)]
pub(in crate::exec) mod test_clock {
    use std::{
        cell::Cell,
        time::{Duration, Instant},
    };

    thread_local! { static CLOCK: Cell<Option<Instant>> = const { Cell::new(None) }; }

    pub(in crate::exec) struct Guard;
    pub(in crate::exec) fn install(at: Instant) -> Guard {
        CLOCK.with(|clock| assert!(clock.replace(Some(at)).is_none()));
        Guard
    }
    pub(in crate::exec) fn now() -> Instant {
        CLOCK.with(|clock| clock.get()).unwrap_or_else(Instant::now)
    }
    pub(in crate::exec) fn set(at: Instant) {
        CLOCK.with(|clock| {
            assert!(clock.get().is_some());
            clock.set(Some(at));
        });
    }
    pub(in crate::exec) fn sleep(duration: Duration) {
        CLOCK.with(|clock| match clock.get() {
            Some(now) => clock.set(Some(now + duration)),
            None => std::thread::sleep(duration),
        });
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            CLOCK.with(|clock| clock.set(None));
        }
    }
}

struct Timing<'a> {
    now: &'a dyn Fn() -> Instant,
    sleep: &'a dyn Fn(Duration),
}
struct Runtime<'a, T> {
    timing: Timing<'a>,
    transport: T,
}

fn await_admission_with<T>(
    peer: AdmissionPeer<'_>,
    initial: &JobRequest,
    inherited: bool,
    claim: Option<&LineageContext>,
    mut deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    mut runtime: Runtime<'_, T>,
) -> Result<Admission, Failure>
where
    T: FnMut(&GovernorJobRequest, Instant, &mut bool) -> Result<GovernorJobResult, Failure>,
{
    let AdmissionPeer {
        id,
        observer,
        route,
        ..
    } = peer;
    let now = runtime.timing.now;
    let sleep = runtime.timing.sleep;
    let mut request = initial.clone();
    let mut waited = false;
    let mut possibly_committed = false;
    loop {
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        let result = (runtime.transport)(
            &GovernorJobRequest::Traffic {
                request: request.clone(),
            },
            deadline,
            &mut possibly_committed,
        );
        match result {
            Ok(GovernorJobResult::Unreserved { job_id, .. }) if &job_id == id => {
                return Ok(Admission::Unreserved { waited })
            }
            Ok(GovernorJobResult::Unconfirmed { .. }) => return Ok(Admission::Unconfirmed),
            Ok(GovernorJobResult::Traffic { result }) => match result {
                JobResult::Granted {
                    permit_generation,
                    lineage_context,
                    ..
                } => {
                    return Ok(Admission::Reserved {
                        generation: permit_generation,
                        context: lineage_context,
                        waited,
                    })
                }
                JobResult::Reused {
                    permit_generation, ..
                } => {
                    return Ok(Admission::Reserved {
                        generation: permit_generation,
                        context: claim.cloned().ok_or(Failure::InvalidReply)?,
                        waited,
                    })
                }
                JobResult::Queued { .. } => {
                    if !waited {
                        write_notice(&format!(
                            "QUEUED\nWaiting to start.\nReference: {}\n",
                            id.as_str()
                        ));
                        waited = true;
                        super::super::launch::after_queue(&json!({"request":request}), cancelled)?;
                    }
                    sleep(Duration::from_millis(40));
                    request = if inherited {
                        initial.clone()
                    } else {
                        JobRequest::Poll {
                            job_id: id.clone(),
                            observer: observer.clone(),
                        }
                    };
                    deadline = now() + Duration::from_millis(1800);
                }
                JobResult::Cancelled {} => return Err(Failure::Cancelled),
                JobResult::NotFound {} if waited || matches!(initial, JobRequest::Poll { .. }) => {
                    return Err(Failure::Uncertain)
                }
                _ => return Err(Failure::InvalidReply),
            },
            Err(Failure::Busy) => {
                if now() >= deadline {
                    return Err(if cancelled() {
                        Failure::Cancelled
                    } else if possibly_committed {
                        Failure::Uncertain
                    } else {
                        Failure::Unavailable
                    });
                }
                if matches!(
                    route,
                    Route::BunScripts | Route::PythonTests | Route::NativeTools
                ) {
                    sleep(Duration::from_millis(20).min(deadline.saturating_duration_since(now())));
                }
            }
            Err(Failure::Unsupported) if possibly_committed => return Err(Failure::Uncertain),
            Err(error) => return Err(error),
            Ok(_) => return Err(Failure::InvalidReply),
        }
    }
}

#[cfg(test)]
mod tests;
