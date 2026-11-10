use super::*;

#[test]
fn queued_enqueue_retry_rejects_stale_owner_before_and_after_reopen() {
    for status in [
        IdentityStatus::Missing,
        IdentityStatus::Mismatch,
        IdentityStatus::Indeterminate,
    ] {
        let db = Database::new();
        let mut c = db.open();
        let policy = Policy::default();
        add(&mut c, 1, "BUILD", 10, &policy);
        add(&mut c, 2, "BUILD", 11, &policy);
        complete_job(&mut c, 1, 1, Outcome::Exited { code: 0 }, 12);
        let stale = OwnerEvidence::simulated(&identity(), status);
        let poll_request = Request::Poll {
            job_id: id(2),
            observer: identity(),
        };
        assert_eq!(
            enqueue(
                &mut c,
                &request(2, "BUILD", None),
                &stale,
                &policy,
                &context(2),
                None,
                13
            ),
            Err(JobError::IdentityUnavailable)
        );
        assert_eq!(permit_counts(&c), (1, 0));
        assert_eq!(queue_head(&c).unwrap().unwrap().job_id, id(2));
        assert_eq!(
            poll(&mut c, &poll_request, &stale, &policy, &context(2), 13),
            Err(JobError::IdentityUnavailable)
        );
        let mut reused_pid = identity();
        reused_pid.start_token.push('9');
        let new_process = OwnerEvidence::simulated(&reused_pid, IdentityStatus::VerifiedMatch);
        assert_eq!(
            poll(
                &mut c,
                &poll_request,
                &new_process,
                &policy,
                &context(2),
                13
            ),
            Err(JobError::InvalidRequest)
        );
        let wrong_owner_request = Request::Poll {
            job_id: id(2),
            observer: reused_pid,
        };
        assert_eq!(
            poll(
                &mut c,
                &wrong_owner_request,
                &new_process,
                &policy,
                &context(2),
                13
            ),
            Err(JobError::Conflict)
        );
        drop(c);
        let mut c = db.open();
        assert_eq!(
            enqueue(
                &mut c,
                &request(2, "BUILD", None),
                &stale,
                &policy,
                &context(2),
                None,
                14
            ),
            Err(JobError::IdentityUnavailable)
        );
        assert_eq!(permit_counts(&c), (1, 0));
        assert_eq!(queue_head(&c).unwrap().unwrap().job_id, id(2));
        assert_eq!(
            poll(&mut c, &poll_request, &stale, &policy, &context(2), 15),
            Err(JobError::IdentityUnavailable)
        );
        let job = get(&c, &id(2)).unwrap().unwrap();
        let observation = QueueObservation::simulated(&job, status, 16);
        let reconciled = reconcile_queued(&mut c, &observation).unwrap();
        if status == IdentityStatus::Indeterminate {
            assert_eq!(reconciled, ReconcileResult::Unchanged);
            assert!(matches!(
                poll_job(&mut c, 2, 17, &policy),
                Reply::Granted { .. }
            ));
        } else {
            assert_eq!(reconciled, ReconcileResult::Abandoned);
            assert_eq!(poll_job(&mut c, 2, 17, &policy), Reply::Abandoned {});
            assert!(matches!(
                add(&mut c, 3, "BUILD", 18, &policy),
                Reply::Granted { .. }
            ));
        }
        assert_eq!(permit_counts(&c), (2, 1));
        drop(c);
        let c = db.open();
        assert_eq!(permit_counts(&c), (2, 1));
    }
}
