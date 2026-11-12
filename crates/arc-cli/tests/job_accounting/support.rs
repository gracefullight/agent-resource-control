use arc_core::job_meter::{
    AccountingRelation, JobMeterReport, JobMeterSummary, QueueWait, SummaryAccounting,
};
use arc_ipc::{
    jobs::*,
    traffic::{JobRequest, JobResult, LineageContext, Resources},
    Request, Response,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs, io,
    os::unix::{
        fs::{symlink, DirBuilderExt, PermissionsExt},
        net::UnixListener,
    },
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Clone, Copy, Debug)]
pub enum PeerMode {
    Normal,
    LostReportAck,
    MalformedReportAck,
    UnsupportedReport,
    StalledReport,
    SlowCompletion,
    SlowBypassThenStalledRegister,
    SlowUnsupportedBunThenStalledRegister,
    Queued,
    UntrackedRoot,
}
#[derive(Debug, Default)]
pub struct State {
    pub reports: HashMap<String, JobMeterReport>,
    pub report_requests: Vec<(String, Value)>,
    pub summary_reads: usize,
    pub attached: bool,
    jobs: HashMap<String, JobSnapshot>,
    relations: HashMap<String, AccountingRelation>,
    contexts: HashMap<String, LineageContext>,
    untracked_root: Option<String>,
}
pub struct Fixture {
    pub root: PathBuf,
    pub socket: PathBuf,
    pub state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Fixture {
    pub fn new(mode: PeerMode) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = arc_core::runtime_root().unwrap().join(format!(
            "ja-{:x}-{:x}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        symlink(env!("CARGO_BIN_EXE_arc-exec-fixture"), root.join("cargo")).unwrap();
        symlink(env!("CARGO_BIN_EXE_arc-exec-fixture"), root.join("bun")).unwrap();
        let socket = root.join("ipc");
        let listener = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (shared, stopping, endpoint) = (state.clone(), stop.clone(), socket.clone());
        let worker = thread::spawn(move || {
            while !stopping.load(Ordering::SeqCst) {
                let (mut stream, _) = match listener.accept() {
                    Ok(stream) => stream,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                // A timed-out client may already have closed this queued
                // connection; Darwin can then reject socket option changes.
                if stream.set_nonblocking(false).is_err()
                    || stream
                        .set_read_timeout(Some(Duration::from_millis(800)))
                        .is_err()
                    || stream
                        .set_write_timeout(Some(Duration::from_millis(800)))
                        .is_err()
                {
                    continue;
                }
                let Ok(request) = arc_ipc::read_message::<_, Request>(&mut stream) else {
                    continue;
                };
                let response = handle(&request, mode, &endpoint, &mut shared.lock().unwrap());
                if let Some(response) = response {
                    let _ = arc_ipc::write_response(&mut stream, &response);
                }
            }
        });
        Self {
            root,
            socket,
            state,
            stop,
            worker: Some(worker),
        }
    }
    pub fn command(&self, scheduled: bool, mode: &str) -> Command {
        self.command_argv(
            if scheduled {
                &["cargo", "build"]
            } else {
                &[env!("CARGO_BIN_EXE_arc-exec-fixture")]
            },
            mode,
        )
    }
    pub fn command_argv(&self, argv: &[&str], mode: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_arcctl"));
        command.args(["exec", "--"]);
        command.args(argv);
        command
            .env(arc_core::ARC_SOCKET_PATH, &self.socket)
            .env("PATH", &self.root)
            .env("ARC_FIXTURE_ACCOUNTING", mode)
            .env("ARC_METER_TEST_MARKER", self.root.join("marker"))
            .env("ARC_METER_TEST_CLI", env!("CARGO_BIN_EXE_arcctl"));
        command
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let result = self.worker.take().unwrap().join();
        let _ = fs::remove_dir_all(&self.root);
        if !std::thread::panicking() {
            result.unwrap();
        }
    }
}
fn registered(
    job_id: JobId,
    observer: ProcessIdentity,
    classification: arc_ipc::Classification,
) -> JobSnapshot {
    JobSnapshot {
        job_id,
        observer,
        classification,
        phase: Phase::Registered,
        observer_verified_at_ms: 1,
        child: None,
        child_verified_at_ms: None,
        observation: IdentityObservation {
            target: ObservationTarget::Observer,
            status: ObservationStatus::VerifiedMatch,
            observed_at_ms: 1,
            reason: LifecycleReason::RegisteredNotClaimed,
        },
        reason: LifecycleReason::RegisteredNotClaimed,
        outcome: None,
        registered_at_ms: 1,
        launch_claimed_at_ms: None,
        completed_at_ms: None,
        updated_at_ms: 1,
        revision: 1,
    }
}
fn claimed(job: &mut JobSnapshot) {
    job.phase = Phase::Launching;
    job.launch_claimed_at_ms = Some(1);
}
fn attached(job: &mut JobSnapshot, child: ProcessIdentity) {
    job.child = Some(child);
    job.child_verified_at_ms = Some(1);
    job.phase = Phase::Running;
    job.reason = LifecycleReason::ChildVerified;
    job.observation = IdentityObservation {
        target: ObservationTarget::Child,
        status: ObservationStatus::VerifiedMatch,
        observed_at_ms: 1,
        reason: job.reason,
    };
}
fn completed(job: &mut JobSnapshot, outcome: Outcome) {
    (job.phase, job.reason) = match &outcome {
        Outcome::Exited { .. } => (Phase::Completed, LifecycleReason::ObservedExit),
        Outcome::Signaled { .. } => (Phase::Signaled, LifecycleReason::ObservedSignal),
        Outcome::SpawnFailed { .. } => (Phase::SpawnFailed, LifecycleReason::ObservedSpawnFailure),
    };
    job.observation.reason = job.reason;
    job.completed_at_ms = Some(1);
    job.outcome = Some(outcome);
}
fn handle(
    request: &Request,
    mode: PeerMode,
    socket: &std::path::Path,
    state: &mut State,
) -> Option<Response> {
    let value = match request.operation.as_str() {
        "traffic.capabilities" => json!({"capability":"traffic.v1"}),
        "traffic.bun.capabilities" => json!({"capability":"traffic.bun.v1"}),
        "job.start" => {
            if matches!(
                mode,
                PeerMode::SlowBypassThenStalledRegister
                    | PeerMode::SlowUnsupportedBunThenStalledRegister
            ) {
                thread::sleep(Duration::from_millis(550));
                return None;
            }
            let input = arc_ipc::jobs::decode_start(&request.payload).unwrap();
            let id = input.job_id().as_str().to_owned();
            if matches!(mode, PeerMode::UntrackedRoot) {
                let root = state.untracked_root.get_or_insert_with(|| id.clone());
                if root == &id {
                    return None;
                }
            }
            match input {
                StartRequest::Register {
                    job_id,
                    observer,
                    classification,
                } => {
                    state
                        .jobs
                        .insert(id.clone(), registered(job_id, observer, classification));
                }
                StartRequest::Claim { .. } => claimed(state.jobs.get_mut(&id).unwrap()),
                StartRequest::Attach {
                    observer, child, ..
                } => {
                    state.relations.insert(
                        id.clone(),
                        arc_platform::job_lineage::relation(socket, &observer, &id),
                    );
                    attached(state.jobs.get_mut(&id).unwrap(), child);
                    state.attached = true;
                }
            }
            assert!(state.jobs[&id].is_valid());
            json!(StartResult {
                disposition: StartDisposition::Applied,
                job: state.jobs[&id].clone()
            })
        }
        "job.complete" => {
            // Decode the original frozen schema: CPU fields must never leak in.
            let input = arc_ipc::jobs::decode_complete(&request.payload).unwrap();
            let job = state.jobs.get_mut(input.job_id.as_str()).unwrap();
            assert_eq!(input.observer, job.observer);
            completed(job, input.outcome);
            assert!(job.is_valid());
            if matches!(mode, PeerMode::SlowCompletion) {
                thread::sleep(Duration::from_millis(550));
            }
            json!(CompleteResult {
                disposition: CompleteDisposition::Applied,
                job: job.clone()
            })
        }
        "traffic.job" | "traffic.bun.job" => {
            let input =
                arc_ipc::traffic::decode_job_for(&request.operation, &request.payload).unwrap();
            let result = match input {
                JobRequest::Enqueue {
                    job_id,
                    observer,
                    classification,
                    lineage_context,
                } => {
                    if matches!(mode, PeerMode::SlowUnsupportedBunThenStalledRegister) {
                        thread::sleep(Duration::from_millis(500));
                        return Some(Response::error(
                            &request.request_id,
                            arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                        ));
                    }
                    if matches!(mode, PeerMode::SlowBypassThenStalledRegister) {
                        thread::sleep(Duration::from_millis(500));
                        JobResult::Bypassed {
                            reason: arc_ipc::traffic::BypassReason::Unsupported,
                        }
                    } else {
                        let id = job_id.as_str().to_owned();
                        let context = lineage_context.unwrap();
                        let mut job = registered(job_id, observer, classification);
                        claimed(&mut job);
                        state.jobs.insert(id.clone(), job);
                        state.contexts.insert(id, context.clone());
                        if matches!(mode, PeerMode::Queued) {
                            return Some(Response::ok(
                                &request.request_id,
                                json!(JobResult::Queued {
                                    sequence: 1,
                                    queued_at_ms: 1,
                                    reason: arc_ipc::traffic::WaitReason::ClassWindow
                                }),
                            ));
                        }
                        JobResult::Granted {
                            sequence: 1,
                            queued_at_ms: 1,
                            granted_at_ms: 1,
                            permit_generation: 1,
                            reservation: Resources {
                                cpu_millis: 1000,
                                memory_bytes: 1024,
                            },
                            lineage_context: context,
                        }
                    }
                }
                JobRequest::Poll { job_id, .. } => {
                    assert!(matches!(mode, PeerMode::Queued));
                    thread::sleep(Duration::from_millis(250));
                    JobResult::Granted {
                        sequence: 1,
                        queued_at_ms: 1,
                        granted_at_ms: 291,
                        permit_generation: 1,
                        reservation: Resources {
                            cpu_millis: 1000,
                            memory_bytes: 1024,
                        },
                        lineage_context: state.contexts[job_id.as_str()].clone(),
                    }
                }
                JobRequest::Attach {
                    job_id,
                    observer,
                    child,
                    ..
                } => {
                    let id = job_id.as_str();
                    state.relations.insert(
                        id.to_owned(),
                        arc_platform::job_lineage::relation(socket, &observer, id),
                    );
                    attached(state.jobs.get_mut(id).unwrap(), child);
                    state.attached = true;
                    JobResult::Running {}
                }
                JobRequest::Complete {
                    job_id, outcome, ..
                } => {
                    completed(
                        state.jobs.get_mut(job_id.as_str()).unwrap(),
                        outcome.clone(),
                    );
                    JobResult::Completed {
                        queue_wait_ms: 0,
                        outcome,
                    }
                }
                _ => panic!("unexpected Traffic request"),
            };
            assert!(result.is_valid());
            json!(result)
        }
        "meter.report" => {
            state
                .report_requests
                .push((request.request_id.clone(), request.payload.clone()));
            if matches!(mode, PeerMode::UnsupportedReport) {
                return Some(Response::error(
                    &request.request_id,
                    arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                ));
            }
            if matches!(mode, PeerMode::StalledReport) {
                thread::sleep(Duration::from_millis(550));
                return None;
            }
            let report = arc_ipc::meter::decode_report(&request.payload).unwrap();
            let Some(job) = state.jobs.get(&report.job_id) else {
                return Some(Response::error(
                    &request.request_id,
                    arc_ipc::meter::ERROR_METER_IDENTITY_MISMATCH,
                ));
            };
            assert_eq!(job.observer, report.observer);
            assert_eq!(job.child.as_ref(), Some(&report.child));
            assert!(arc_ipc::meter::matches_terminal_outcome(
                &report,
                job.outcome.as_ref().unwrap()
            ));
            assert_eq!(
                state.relations.get(&report.job_id),
                Some(&report.accounting_relation)
            );
            let duplicate = state.reports.get(&report.job_id);
            if let Some(first) = duplicate {
                assert_eq!(first, &report);
            }
            let disposition = if duplicate.is_some() {
                "unchanged"
            } else {
                "accepted"
            };
            let mut receipt = json!({"schema_version":1, "job_id":report.job_id, "report_id":report.report_id, "disposition":disposition});
            let first = duplicate.is_none();
            state.reports.insert(report.job_id.clone(), report);
            if first && matches!(mode, PeerMode::LostReportAck) {
                return None;
            }
            if first && matches!(mode, PeerMode::MalformedReportAck) {
                receipt["report_id"] = "peer-secret-canary".into();
            }
            receipt
        }
        "meter.summary" => {
            state.summary_reads += 1;
            let input = arc_ipc::meter::decode_summary_request(&request.payload).unwrap();
            let accounting =
                state
                    .reports
                    .get(&input.job_id)
                    .map_or(SummaryAccounting::Missing {}, |r| {
                        SummaryAccounting::Recorded {
                            report: Box::new(r.clone()),
                        }
                    });
            let summary = JobMeterSummary {
                schema_version: 1,
                job_id: input.job_id,
                queue_wait: if matches!(accounting, SummaryAccounting::Missing {}) {
                    QueueWait::unknown()
                } else {
                    QueueWait::lifecycle_only()
                },
                accounting,
            };
            assert!(summary.is_valid());
            json!(summary)
        }
        _ => {
            return Some(Response::error(
                &request.request_id,
                arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
            ))
        }
    };
    Some(Response::ok(&request.request_id, value))
}
