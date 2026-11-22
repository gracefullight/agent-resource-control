use super::*;

pub(super) fn handle(
    request: &Request,
    mode: PeerMode,
    socket: &std::path::Path,
    state: &mut State,
    queued: Option<&QueuedPeer>,
    attachments: &Sender<()>,
) -> Option<Response> {
    let value = match request.operation.as_str() {
        "traffic.capabilities" => json!({"capability":"traffic.v1"}),
        "traffic.bun.capabilities" => json!({"capability":"traffic.bun.v1"}),
        "job.start" => {
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
                    capture_relation(state, socket, &observer, &id);
                    attached(state.jobs.get_mut(&id).unwrap(), child);
                    state.attached = true;
                    let _ = attachments.send(());
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
                    project: _,
                } => {
                    if matches!(mode, PeerMode::UnsupportedBunThenUnavailable) {
                        return Some(Response::error(
                            &request.request_id,
                            arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                        ));
                    }
                    if matches!(mode, PeerMode::BypassThenUnavailable) {
                        JobResult::Bypassed {
                            reason: arc_ipc::traffic::BypassReason::Unsupported,
                        }
                    } else {
                        let id = job_id.as_str().to_owned();
                        let context = lineage_context.unwrap();
                        if matches!(mode, PeerMode::Queued | PeerMode::QueuedExpireReport)
                            && state.jobs.contains_key(&id)
                        {
                            assert_eq!(state.jobs[&id].observer, observer);
                            assert_eq!(state.contexts[&id], context);
                            let result = queued_result(&job_id, state, queued.unwrap())?;
                            return Some(Response::ok(&request.request_id, json!(result)));
                        }
                        let mut job = registered(job_id, observer, classification);
                        claimed(&mut job);
                        state.jobs.insert(id.clone(), job);
                        state.contexts.insert(id, context.clone());
                        if matches!(mode, PeerMode::Queued | PeerMode::QueuedExpireReport) {
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
                    assert!(matches!(
                        mode,
                        PeerMode::Queued | PeerMode::QueuedExpireReport
                    ));
                    queued_result(&job_id, state, queued.unwrap())?
                }
                JobRequest::Attach {
                    job_id,
                    observer,
                    child,
                    ..
                } => {
                    let id = job_id.as_str();
                    capture_relation(state, socket, &observer, id);
                    attached(state.jobs.get_mut(id).unwrap(), child);
                    state.attached = true;
                    let _ = attachments.send(());
                    JobResult::Running {}
                }
                JobRequest::Complete {
                    job_id, outcome, ..
                } => {
                    completed(
                        state.jobs.get_mut(job_id.as_str()).unwrap(),
                        outcome.clone(),
                    );
                    if matches!(mode, PeerMode::QueuedExpireReport) {
                        let clock = socket.parent().unwrap().join("delivery-clock");
                        let pending = socket.parent().unwrap().join("delivery-clock.pending");
                        fs::write(&pending, b"501").unwrap();
                        fs::rename(pending, clock).unwrap();
                    }
                    JobResult::Completed {
                        queue_wait_ms: 0,
                        outcome,
                    }
                }
                unexpected => panic!("unexpected Traffic request in {mode:?}: {unexpected:?}"),
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
