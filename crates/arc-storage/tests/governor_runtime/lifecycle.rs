use super::*;

#[test]
fn unreserved_completion_retains_outcome_wait_and_has_no_permit() {
    let mut c = db();
    let normal = observation::Provider::new(arc_core::meter::MemoryPressure::Normal);
    set(&mut c, &intent(0, false, 1), 1);
    enqueue(&mut c, 1, &normal);
    let token = format!("{:032x}", 1);
    let tx = c.transaction().unwrap();
    execution::claim(
        &tx,
        &id(1),
        &owner(),
        &traffic::OwnerEvidence::gather(&owner()),
        &token,
        20,
    )
    .unwrap();
    tx.commit().unwrap();
    let expected = Outcome::Exited { code: 7 };
    let result = execution::complete(&mut c, &id(1), &owner(), &token, &expected, 30).unwrap();
    assert!(matches!(
        result,
        GovernorJobResult::Traffic {
            result: Legacy::Completed {
                queue_wait_ms: 0,
                outcome: Outcome::Exited { code: 7 }
            }
        }
    ));
    assert_eq!(
        execution::complete(&mut c, &id(1), &owner(), &token, &expected, 31).unwrap(),
        result
    );
    assert_eq!(
        execution::complete(
            &mut c,
            &id(1),
            &owner(),
            &token,
            &Outcome::Exited { code: 8 },
            31
        ),
        Err(JobError::Conflict)
    );
    assert_eq!(count(&c, "SELECT count(*) FROM permits"), 0);
}

#[test]
fn converted_queue_cancel_and_legacy_off_projection_preserve_owned_work() {
    let mut c = db();
    let normal = observation::Provider::new(arc_core::meter::MemoryPressure::Normal);
    enqueue(&mut c, 1, &normal);
    enqueue(&mut c, 2, &normal);
    set(&mut c, &intent(0, false, 1), 20);
    let cancel = JobRequest::Cancel {
        job_id: id(2),
        observer: owner(),
        permit_generation: None,
    };
    assert_eq!(
        traffic::cancel_observed(
            &mut c,
            &cancel,
            &traffic::OwnerEvidence::gather(&owner()),
            21,
            &normal
        )
        .unwrap(),
        Legacy::Cancelled {}
    );
    assert_eq!(
        traffic::cancel_observed(
            &mut c,
            &cancel,
            &traffic::OwnerEvidence::gather(&owner()),
            22,
            &normal
        )
        .unwrap(),
        Legacy::Cancelled {}
    );
    let tx = c.transaction().unwrap();
    assert_eq!(
        execution::claim(
            &tx,
            &id(2),
            &owner(),
            &traffic::OwnerEvidence::gather(&owner()),
            &format!("{:032x}", 2),
            23
        )
        .unwrap(),
        GovernorLaunchResult::Cancelled {}
    );
    tx.commit().unwrap();
    let new=JobRequest::Enqueue{job_id:id(3),observer:owner(),classification:serde_json::from_value(serde_json::json!({"classification":"BUILD","locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"})).unwrap(),lineage_context:None};
    assert!(matches!(
        traffic::enqueue(
            &mut c,
            &new,
            &traffic::OwnerEvidence::gather(&owner()),
            &arc_traffic::Policy::default(),
            &LineageContext::try_from(format!("{:064x}", 3)).unwrap(),
            None,
            24
        )
        .unwrap(),
        Legacy::Bypassed {
            reason: arc_ipc::traffic::BypassReason::Unsupported
        }
    ));
    assert!(arc_storage::jobs::get(&c, &id(3)).unwrap().is_none());
    assert_eq!(
        count(&c, "SELECT count(*) FROM permits WHERE state='ACTIVE'"),
        0
    );
}

#[test]
fn failed_commit_rolls_back_switch_dispositions_receipt_and_reservation() {
    let mut c = db();
    let normal = observation::Provider::new(arc_core::meter::MemoryPressure::Normal);
    enqueue(&mut c, 1, &normal);
    {
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        assert!(matches!(
            state::set_switch(&tx, &intent(0, false, 1), 20).unwrap(),
            GovernorMutationResult::Committed { .. }
        ));
        tx.execute_batch("PRAGMA defer_foreign_keys=ON").unwrap();
        tx.execute(
            "INSERT INTO governor_jobs(job_id,mode,switch_revision) VALUES('absent','SCHEDULED',1)",
            [],
        )
        .unwrap();
        assert!(tx.commit().is_err());
    }
    assert!(state::get_switch(&c).unwrap().enabled);
    assert_eq!(state::get_switch(&c).unwrap().revision, 0);
    assert_eq!(
        count(&c, "SELECT count(*) FROM permits WHERE state='ACTIVE'"),
        1
    );
    assert_eq!(
        count(
            &c,
            "SELECT count(*) FROM governor_jobs WHERE mode='UNRESERVED'"
        ),
        0
    );
    assert_eq!(
        count(&c, "SELECT count(*) FROM governor_mutation_receipts"),
        0
    );
}
#[test]
fn unreserved_queue_wait_and_recovery_candidate_use_release_key_without_a_permit() {
    let mut c = db();
    let normal = observation::Provider::new(arc_core::meter::MemoryPressure::Normal);
    set(&mut c, &intent(0, false, 1), 1);
    enqueue(&mut c, 1, &normal);
    let token = format!("{:032x}", 1);
    assert_eq!(
        execution::claim(
            &c,
            &id(1),
            &owner(),
            &traffic::OwnerEvidence::gather(&owner()),
            &token,
            20
        ),
        Err(JobError::InvalidTransition)
    );
    let tx = c.transaction().unwrap();
    execution::claim(
        &tx,
        &id(1),
        &owner(),
        &traffic::OwnerEvidence::gather(&owner()),
        &token,
        20,
    )
    .unwrap();
    tx.commit().unwrap();
    let candidates = traffic::recovery_candidates(&c, 0, 64).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].witness_context.as_ref().unwrap().as_str(),
        format!("{token}{token}")
    );
    let summary = arc_storage::job_meter::summary(&mut c, id(1).as_str(), 21).unwrap();
    assert_eq!(summary.queue_wait.seconds, Some(0.0));
    assert_eq!(count(&c, "SELECT count(*) FROM permits"), 0);
}

#[test]
fn late_reserved_unspawned_ack_reconciles_pause_without_an_extra_poll() {
    let mut c = db();
    let normal = observation::Provider::new(arc_core::meter::MemoryPressure::Normal);
    let GovernorJobResult::Traffic {
        result: Legacy::Granted {
            permit_generation, ..
        },
    } = enqueue(&mut c, 1, &normal)
    else {
        panic!()
    };
    let request = arc_ipc::policy::launch::LaunchRequest {
        job_id: id(1),
        observer: owner(),
        permit_generation,
    };
    let tx = c.transaction().unwrap();
    traffic::launch(
        &tx,
        &request,
        &traffic::OwnerEvidence::gather(&owner()),
        None,
        &normal.policy(),
        11,
    )
    .unwrap();
    tx.commit().unwrap();
    set(&mut c, &intent(0, false, 1), 12);
    assert_eq!(
        count(&c, "SELECT count(*) FROM permits WHERE state='ACTIVE'"),
        1
    );
    traffic::acknowledge_unspawned_launch(
        &mut c,
        &request,
        &traffic::OwnerEvidence::gather(&owner()),
    )
    .unwrap();
    assert_eq!(
        count(&c, "SELECT count(*) FROM permits WHERE state='ACTIVE'"),
        0
    );
    assert!(
        execution::disposition(&c, &id(1))
            .unwrap()
            .unwrap()
            .unreserved
    );
}

fn late_unreserved_ack_preserves_terminal(terminal: &str, resume: bool) {
    let mut c = db();
    let normal = observation::Provider::new(arc_core::meter::MemoryPressure::Normal);
    let observer = owner();
    let evidence = traffic::OwnerEvidence::gather(&observer);
    assert!(matches!(
        enqueue(&mut c, 1, &normal),
        GovernorJobResult::Traffic {
            result: Legacy::Granted { .. }
        }
    ));
    set(&mut c, &intent(0, false, 1), 20);
    let token = format!("{:032x}", 1);
    let tx = c.transaction().unwrap();
    assert!(matches!(
        execution::claim(&tx, &id(1), &observer, &evidence, &token, 21).unwrap(),
        GovernorLaunchResult::Unreserved { .. }
    ));
    tx.commit().unwrap();
    assert_eq!(
        traffic::cancel_observed(
            &mut c,
            &JobRequest::Cancel {
                job_id: id(1),
                observer: observer.clone(),
                permit_generation: None,
            },
            &evidence,
            22,
            &normal,
        )
        .unwrap(),
        Legacy::Cancelled {}
    );
    if terminal == "ABANDONED" {
        // Explicit persisted-state fixture for recovery's other terminal shape.
        // Cancellation above exercises the real public transition, with neither
        // a child nor a lifecycle outcome to protect against a late ACK reset.
        c.execute(
            "UPDATE traffic_entries SET state='ABANDONED' WHERE job_id=?1",
            [id(1).as_str()],
        )
        .unwrap();
    }
    let before = arc_storage::jobs::get(&c, &id(1)).unwrap().unwrap();
    assert!(before.child.is_none() && before.outcome.is_none());
    if resume {
        set(&mut c, &intent(1, true, 2), 23);
    }
    assert_eq!(
        execution::unspawned(
            &mut c,
            &id(1),
            &observer,
            &format!("{:032x}", 2),
            &evidence,
            24,
        ),
        Err(JobError::Conflict)
    );
    assert_eq!(
        execution::disposition(&c, &id(1))
            .unwrap()
            .unwrap()
            .release_id,
        Some(token.clone()),
        "an unmatched ACK must not consume the terminal job's claim"
    );
    execution::unspawned(&mut c, &id(1), &observer, &token, &evidence, 25).unwrap();
    let status = if terminal == "CANCELLED" {
        arc_ipc::traffic::StatusResult::Cancelled {}
    } else {
        arc_ipc::traffic::StatusResult::Abandoned {}
    };
    assert_eq!(traffic::status(&c, &id(1)).unwrap(), status);
    assert_eq!(arc_storage::jobs::get(&c, &id(1)).unwrap().unwrap(), before);
    assert!(execution::disposition(&c, &id(1))
        .unwrap()
        .unwrap()
        .release_id
        .is_none());
    // A consumed ACK cannot be replayed into another transition.
    assert_eq!(
        execution::unspawned(&mut c, &id(1), &observer, &token, &evidence, 26),
        Err(JobError::Conflict)
    );
    for now in [27, 28] {
        let expected = if terminal == "CANCELLED" {
            Legacy::Cancelled {}
        } else {
            Legacy::Abandoned {}
        };
        assert_eq!(
            execution::poll(
                &mut c,
                &JobRequest::Poll {
                    job_id: id(1),
                    observer: observer.clone(),
                },
                &evidence,
                &arc_traffic::Policy::default(),
                &LineageContext::try_from(format!("{now:064x}")).unwrap(),
                None,
                now,
                &normal,
            )
            .unwrap(),
            GovernorJobResult::Traffic { result: expected }
        );
        let tx = c.transaction().unwrap();
        assert_eq!(
            execution::claim(&tx, &id(1), &observer, &evidence, &token, now).unwrap(),
            GovernorLaunchResult::Cancelled {}
        );
        tx.commit().unwrap();
    }
    assert_eq!(traffic::status(&c, &id(1)).unwrap(), status);
    assert_eq!(arc_storage::jobs::get(&c, &id(1)).unwrap().unwrap(), before);
    let permits = count(&c, "SELECT count(*) FROM permits");
    let active = count(&c, "SELECT count(*) FROM permits WHERE state='ACTIVE'");
    let claims = count(&c, "SELECT count(*) FROM governor_jobs WHERE release_id IS NOT NULL OR release_claimed_at_ms IS NOT NULL");
    let children = count(
        &c,
        "SELECT count(*) FROM job_lifecycle WHERE child_pid IS NOT NULL",
    );
    assert_eq!((permits, active, claims, children), (1, 0, 0, 0));
    eprintln!(
        "governor-terminal-ack-observation={}",
        serde_json::json!({"terminal":terminal,"interveningResume":resume,
            "switch":state::get_switch(&c).unwrap(),"status":status,
            "permits":{"total":permits,"active":active},"releaseClaims":claims,"sqliteChildren":children,
            "scope":"storage transaction regression; ABANDONED is an explicit persisted-state fixture"})
    );
}

#[test]
fn late_unreserved_ack_preserves_cancelled_with_and_without_resume() {
    for resume in [false, true] {
        late_unreserved_ack_preserves_terminal("CANCELLED", resume);
    }
}

#[test]
fn late_unreserved_ack_preserves_abandoned_with_and_without_resume() {
    for resume in [false, true] {
        late_unreserved_ack_preserves_terminal("ABANDONED", resume);
    }
}
