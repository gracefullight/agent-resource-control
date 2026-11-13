//! Regressions for task-6's independently reviewed launch and lineage races.
use super::*;
use arc_ipc::traffic::JobResult as Legacy;

fn reserved(f: &Fixture, n: u32, generation: u64) -> GovernorLaunchRequest {
    GovernorLaunchRequest::Reserved {
        request: arc_ipc::policy::launch::LaunchRequest {
            job_id: f.id(n),
            observer: f.owner.clone(),
            permit_generation: generation,
        },
    }
}

fn generation(value: Value) -> u64 {
    match value {
        Value::Job(GovernorJobResult::Traffic {
            result: Legacy::Granted {
                permit_generation, ..
            },
        }) => permit_generation,
        _ => panic!("expected root grant"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_resumed_old_reserved_launch_retries_then_one_child_obeys_restored_rules() {
    let f = Fixture::new(None).await;
    let old = generation(f.governor(enqueue(&f, 1)).await);
    assert!(matches!(
        f.governor(enqueue(&f, 2)).await,
        Value::Job(GovernorJobResult::Traffic {
            result: Legacy::Queued { .. }
        })
    ));
    set(&f, intent(0, false, 1)).await;
    assert_eq!(counts(&f), (1, 0));
    set(&f, intent(1, true, 2)).await;
    f.level(1).await;
    let stale = reserved(&f, 1, old);
    assert!(matches!(
        f.governor(Input::Launch(stale.clone())).await,
        Value::Launch {
            result: GovernorLaunchResult::Retry {},
            fence: None
        }
    ));
    assert!(matches!(
        poll(&f, 1).await,
        GovernorJobResult::Traffic {
            result: Legacy::Queued { .. }
        }
    ));
    assert_eq!(counts(&f), (1, 0), "Critical must prevent readmission");
    f.level(0).await;
    let fresh = generation(Value::Job(poll(&f, 1).await));
    assert!(fresh > old);
    assert!(matches!(
        poll(&f, 2).await,
        GovernorJobResult::Traffic {
            result: Legacy::Queued { .. }
        }
    ));
    assert!(matches!(
        f.governor(Input::Launch(stale.clone())).await,
        Value::Launch {
            result: GovernorLaunchResult::Retry {},
            fence: None
        }
    ));
    let current = reserved(&f, 1, fresh);
    let Value::Launch {
        result: GovernorLaunchResult::Reserved { result },
        fence,
    } = f.governor(Input::Launch(current.clone())).await
    else {
        panic!("fresh generation must launch");
    };
    assert!(matches!(
        *result,
        arc_ipc::policy::launch::LaunchResult::Ready { .. }
    ));
    let child = Child::spawn();
    let identity = child.identity();
    drop(fence);
    for request in [stale, current] {
        assert!(matches!(
            f.governor(Input::Launch(request)).await,
            Value::Launch {
                result: GovernorLaunchResult::Unconfirmed {},
                ..
            }
        ));
    }
    f.traffic(JobRequest::Attach {
        job_id: f.id(1),
        observer: f.owner.clone(),
        child: identity.clone(),
        permit_generation: fresh,
    })
    .await;
    let outcome = child.finish();
    f.traffic(JobRequest::Complete {
        job_id: f.id(1),
        observer: f.owner.clone(),
        permit_generation: fresh,
        outcome: outcome.clone(),
    })
    .await;
    f.traffic(JobRequest::Cancel {
        job_id: f.id(2),
        observer: f.owner.clone(),
        permit_generation: None,
    })
    .await;
    assert_eq!(counts(&f), (1, 0));
    let db = ro(&f);
    let children: u32 = db
        .query_row(
            "SELECT count(*) FROM job_lifecycle WHERE child_pid IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(children, 1);
    let job = arc_storage::jobs::get(&db, &f.id(1)).unwrap().unwrap();
    assert_eq!(job.child.as_ref(), Some(&identity));
    assert_eq!(job.outcome.as_ref(), Some(&outcome));
    let switch = state::get_switch(&db).unwrap();
    assert!(switch.enabled);
    assert_eq!(switch.revision, 2);
    let permits = counts(&f);
    eprintln!(
        "governor-repair-observation={}",
        serde_json::json!({
            "case":"reserved-pause-resume", "switchRevision":2, "oldGeneration":old,
            "freshGeneration":fresh, "sqliteChildren":children, "child":identity,
            "outcome":outcome, "switch":switch,
            "permits":{"total":permits.0,"active":permits.1},
            "scope":"production handler/writer with explicit injected pressure"
        })
    );
    drop(db);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_resume_never_retries_a_possibly_delivered_unreserved_claim() {
    let f = Fixture::new(None).await;
    let old = generation(f.governor(enqueue(&f, 1)).await);
    set(&f, intent(0, false, 1)).await;
    let request = reserved(&f, 1, old);
    let Value::Launch {
        result: result @ GovernorLaunchResult::Unreserved { .. },
        fence,
    } = f.governor(Input::Launch(request.clone())).await
    else {
        panic!("release")
    };
    drop(fence); // Lost acknowledgment is not evidence of no spawn.
    let before = execution::disposition(&ro(&f), &f.id(1))
        .unwrap()
        .unwrap()
        .release_id;
    assert!(before.is_some());
    set(&f, intent(1, true, 2)).await;
    assert!(matches!(
        f.governor(Input::Launch(request.clone())).await,
        Value::Launch {
            result: GovernorLaunchResult::Unconfirmed {},
            ..
        }
    ));
    assert!(matches!(
        poll(&f, 1).await,
        GovernorJobResult::Unconfirmed { .. }
    ));
    assert_eq!(
        execution::disposition(&ro(&f), &f.id(1))
            .unwrap()
            .unwrap()
            .release_id,
        before
    );
    assert_eq!(counts(&f), (1, 0));
    f.governor(Input::Unspawned {
        request: request.clone(),
        result,
    })
    .await;
    assert!(matches!(
        f.governor(Input::Launch(request)).await,
        Value::Launch {
            result: GovernorLaunchResult::Retry {},
            fence: None
        }
    ));
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_enqueue_refreshes_lineage_after_anchor_attaches() {
    let f = Fixture::new(None).await;
    let root_generation = generation(f.governor(enqueue(&f, 1)).await);
    let child = Child::spawn();
    let nested_owner = child.identity();
    let Input::Job(GovernorJobRequest::Traffic {
        request: JobRequest::Enqueue { classification, .. },
    }) = enqueue(&f, 2)
    else {
        unreachable!()
    };
    let request = JobRequest::Enqueue {
        job_id: f.id(2),
        observer: nested_owner.clone(),
        classification,
        lineage_context: None,
    };
    let attachment = JobRequest::Attach {
        job_id: f.id(1),
        observer: f.owner.clone(),
        child: nested_owner.clone(),
        permit_generation: root_generation,
    };
    let mut other = arc_storage::open_and_migrate(f.path()).unwrap();
    let mut proofs = 0;
    let reply = handle_inner(
        f.client.clone(),
        Input::Job(GovernorJobRequest::Traffic {
            request: request.clone(),
        }),
        nested_owner.pid,
        Instant::now() + Duration::from_secs(5),
        |has_proof| {
            assert!(has_proof);
            proofs += 1;
            if proofs == 1 {
                assert!(matches!(
                    repository::attach(
                        &mut other,
                        &attachment,
                        &repository::ChildEvidence::gather(&nested_owner),
                        jobs::now(),
                    )
                    .unwrap(),
                    Legacy::Running {}
                ));
            }
        },
    )
    .await
    .unwrap();
    assert_eq!(
        proofs, 2,
        "stale durable revision must cause fresh native proof"
    );
    assert!(matches!(reply, Value::Job(GovernorJobResult::Traffic {
        result: Legacy::Reused { permit_generation, .. }
    }) if permit_generation == root_generation));
    assert!(matches!(
        handle(
            f.client.clone(),
            Input::Job(GovernorJobRequest::Traffic { request }),
            nested_owner.pid,
            Instant::now() + Duration::from_secs(5),
        )
        .await
        .unwrap(),
        Value::Job(GovernorJobResult::Traffic {
            result: Legacy::Reused { .. }
        })
    ));
    assert_eq!(counts(&f), (1, 1));
    eprintln!(
        "governor-repair-observation={}",
        serde_json::json!({"case":"lineage-attachment-refresh", "proofCaptures":proofs,
            "result":"REUSED", "permitGeneration":root_generation,
            "permitsBeforeCleanup":counts(&f),
            "scope":"Governor handler with deterministic anchor attachment between proof and write"})
    );
    let outcome = child.finish();
    for (n, observer) in [(1, &f.owner), (2, &nested_owner)] {
        repository::complete(
            &mut other,
            &JobRequest::Complete {
                job_id: f.id(n),
                observer: observer.clone(),
                permit_generation: root_generation,
                outcome: outcome.clone(),
            },
            jobs::now(),
        )
        .unwrap();
    }
    assert_eq!(counts(&f), (1, 0));
    drop(other);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_poll_refreshes_lineage_after_root_permit_releases() {
    let f = Fixture::new(None).await;
    let root_generation = generation(f.governor(enqueue(&f, 1)).await);
    let child = Child::spawn();
    let nested_owner = child.identity();
    let Input::Job(GovernorJobRequest::Traffic {
        request: JobRequest::Enqueue { classification, .. },
    }) = enqueue(&f, 2)
    else {
        unreachable!()
    };
    let mut other = arc_storage::open_and_migrate(f.path()).unwrap();
    // Register before native ancestry discovery, then exercise Governor Poll.
    assert!(matches!(
        execution::enqueue(
            &mut other,
            &JobRequest::Enqueue {
                job_id: f.id(2),
                observer: nested_owner.clone(),
                classification,
                lineage_context: None,
            },
            &repository::OwnerEvidence::gather(&nested_owner),
            &arc_traffic::Policy::default(),
            &traffic::context().unwrap(),
            None,
            jobs::now(),
            &arc_policy::evaluator::MissingObservation,
        )
        .unwrap(),
        GovernorJobResult::Traffic {
            result: Legacy::Queued { .. }
        }
    ));
    let mut captured = Vec::new();
    let reply = handle_inner(
        f.client.clone(),
        Input::Job(GovernorJobRequest::Traffic {
            request: JobRequest::Poll {
                job_id: f.id(2),
                observer: nested_owner.clone(),
            },
        }),
        nested_owner.pid,
        Instant::now() + Duration::from_secs(5),
        |has_proof| {
            captured.push(has_proof);
            if captured.len() == 1 {
                assert!(has_proof);
                // Native owner remains alive; a completed root is no longer an anchor.
                repository::complete(
                    &mut other,
                    &JobRequest::Complete {
                        job_id: f.id(1),
                        observer: f.owner.clone(),
                        permit_generation: root_generation,
                        outcome: Outcome::SpawnFailed {
                            reason: arc_ipc::jobs::SpawnReason::ResourceUnavailable,
                            os_code: None,
                        },
                    },
                    jobs::now(),
                )
                .unwrap();
            }
        },
    )
    .await
    .unwrap();
    assert_eq!(captured, [true, false]);
    let fresh = generation(reply);
    assert_ne!(fresh, root_generation);
    assert_eq!(counts(&f), (2, 1));
    eprintln!(
        "governor-repair-observation={}",
        serde_json::json!({"case":"lineage-release-refresh", "proofCaptures":captured,
            "result":"GRANTED", "releasedGeneration":root_generation,
            "freshGeneration":fresh, "permitsBeforeCleanup":counts(&f),
            "scope":"Governor Poll with injected pre-spawn failure between proof and write"})
    );
    repository::cancel_observed(
        &mut other,
        &JobRequest::Cancel {
            job_id: f.id(2),
            observer: nested_owner,
            permit_generation: Some(fresh),
        },
        &repository::OwnerEvidence::gather(&child.identity()),
        jobs::now(),
        &arc_policy::evaluator::MissingObservation,
    )
    .unwrap();
    assert_eq!(counts(&f), (2, 0));
    drop(child);
    drop(other);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_completed_unreserved_attach_replay_projects_terminal_outcome() {
    let f = Fixture::new(None).await;
    set(&f, intent(0, false, 1)).await;
    f.governor(enqueue(&f, 1)).await;
    let Value::Launch {
        result: GovernorLaunchResult::Unreserved { release_id, .. },
        fence,
    } = f
        .governor(Input::Launch(GovernorLaunchRequest::Unreserved {
            job_id: f.id(1),
            observer: f.owner.clone(),
        }))
        .await
    else {
        panic!("unreserved launch")
    };
    let child = Child::spawn();
    let identity = child.identity();
    drop(fence);
    let attach = GovernorJobRequest::Attach {
        job_id: f.id(1),
        observer: f.owner.clone(),
        release_id: release_id.clone(),
        child: identity.clone(),
    };
    assert!(matches!(
        f.governor(Input::Job(attach.clone())).await,
        Value::Job(GovernorJobResult::Traffic {
            result: Legacy::Running {}
        })
    ));
    // An authenticated replay can be delayed in the writer after this native
    // evidence was gathered, and execute after the child completes.
    let evidence = repository::ChildEvidence::gather(&identity);
    let outcome = child.finish();
    let complete = GovernorJobRequest::Complete {
        job_id: f.id(1),
        observer: f.owner.clone(),
        release_id,
        outcome: outcome.clone(),
    };
    let Value::Job(expected) = f.governor(Input::Job(complete.clone())).await else {
        panic!()
    };
    assert!(matches!(&expected, GovernorJobResult::Traffic {
        result: Legacy::Completed { outcome: actual, .. }
    } if actual == &outcome));
    let before = arc_storage::jobs::get(&ro(&f), &f.id(1)).unwrap().unwrap();
    for operation in [
        Operation::Attach(attach.clone(), evidence.clone()),
        Operation::Complete(complete),
        Operation::Attach(attach, evidence),
    ] {
        let Value::Job(actual) = call(
            &f.client,
            operation,
            Instant::now() + Duration::from_secs(5),
        )
        .await
        .unwrap() else {
            panic!()
        };
        assert_eq!(
            actual, expected,
            "replay cannot report a completed child as running"
        );
    }
    assert_eq!(
        arc_storage::jobs::get(&ro(&f), &f.id(1)).unwrap().unwrap(),
        before
    );
    assert_eq!(counts(&f), (0, 0));
    let releases: u32 = ro(&f)
        .query_row(
            "SELECT count(*) FROM governor_jobs WHERE release_id IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(releases, 1);
    let permits = counts(&f);
    eprintln!(
        "governor-repair-observation={}",
        serde_json::json!({
            "case":"terminal-attach-replay", "child":identity, "outcome":outcome,
            "replies":expected, "lifecycleRevision":before.revision,
            "releaseClaims":releases, "permits":{"total":permits.0,"active":permits.1},
            "scope":"production handler/writer"
        })
    );
    f.shutdown().await;
}
