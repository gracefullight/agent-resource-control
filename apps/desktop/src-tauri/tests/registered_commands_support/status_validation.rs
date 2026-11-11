#[test]
fn registered_status_rejects_wrong_variants_duplicates_limits_and_unsafe_times() {
    let mut invalid_recent = Vec::new();
    for (field, value) in [
        ("liveness_fresh", json!(true)),
        ("observed_at_ms", json!(MAX_TIMESTAMP + 1)),
        ("jobs", json!(vec![summary(); 51])),
        ("jobs", json!([summary(), summary()])),
        ("kind", json!("FOUND")),
    ] {
        let mut value_recent = recent();
        value_recent[field] = value;
        invalid_recent.push(value_recent);
    }
    let mut invalid = recent();
    invalid["jobs"][0]["registered_at_ms"] = json!(151);
    invalid_recent.push(invalid);
    let mut invalid = recent();
    invalid["jobs"][0]["updated_at_ms"] = json!(MAX_TIMESTAMP + 1);
    invalid_recent.push(invalid);
    invalid_recent.push(json!({"kind":"NOT_FOUND"}));
    for result in invalid_recent {
        let peer = Peer::result(result);
        assert_eq!(
            App::new(peer.bridge()).invoke("recent_jobs", json!({})),
            Err(error("invalid_reply", false))
        );
    }
    let peer = Peer::result(recent());
    assert_eq!(
        App::new(peer.bridge()).invoke("job_by_id", json!({"job_id":JOB})),
        Err(error("invalid_reply", false))
    );
    let mut result = found(snapshot(17));
    result["job"]["job_id"] = json!(OTHER_JOB);
    let peer = Peer::result(result);
    assert_eq!(
        App::new(peer.bridge()).invoke("job_by_id", json!({"job_id":JOB})),
        Err(error("invalid_reply", false))
    );
}

#[test]
fn registered_status_validates_full_snapshot_before_projection() {
    let valid = found(snapshot(0));
    let mut invalid_results = Vec::new();
    for field in [
        "outcome",
        "completed_at_ms",
        "launch_claimed_at_ms",
        "child",
        "child_verified_at_ms",
    ] {
        let mut result = valid.clone();
        result["job"].as_object_mut().unwrap().remove(field);
        invalid_results.push(result);
    }
    for (field, value) in [
        ("revision", json!(0)),
        ("revision", json!(MAX_TIMESTAMP + 1)),
        ("outcome", Value::Null),
        ("outcome", json!({"kind":"EXITED","code":256})),
        (
            "outcome",
            json!({"kind":"SIGNALED","signal":0,"core_dumped":false}),
        ),
        ("observer_verified_at_ms", json!(MAX_TIMESTAMP + 1)),
        ("classification", json!({"command":SENTINEL})),
        ("completed_at_ms", json!(109)),
        ("outcome", json!(["EXITED", 0])),
        ("observer", json!({"pid":0})),
        (
            "observation",
            json!(["CHILD", "MISSING", 150, "OBSERVED_EXIT"]),
        ),
    ] {
        let mut result = valid.clone();
        result["job"][field] = value;
        invalid_results.push(result);
    }
    for (index, result) in invalid_results.into_iter().enumerate() {
        let peer = Peer::result(result);
        let response = App::new(peer.bridge()).invoke("job_by_id", json!({"job_id":JOB}));
        assert!(
            response.is_err(),
            "invalid snapshot fixture {index} was accepted"
        );
        let rejected = response.unwrap_err();
        assert_eq!(rejected, error("invalid_reply", false));
        assert!(!rejected.to_string().contains(SENTINEL));
    }
    // Distinct recorded signal/spawn outcomes pass unchanged, never as exit 0.
    let mut signaled = snapshot(0);
    signaled.phase = Phase::Signaled;
    signaled.reason = LifecycleReason::ObservedSignal;
    signaled.observation.reason = signaled.reason;
    signaled.outcome = Some(Outcome::Signaled {
        signal: 15,
        core_dumped: false,
    });
    let mut failed = snapshot(0);
    failed.phase = Phase::SpawnFailed;
    failed.reason = LifecycleReason::ObservedSpawnFailure;
    failed.observation.reason = failed.reason;
    failed.observation.target = ObservationTarget::Observer;
    failed.observation.status = ObservationStatus::VerifiedMatch;
    failed.child = None;
    failed.child_verified_at_ms = None;
    failed.outcome = Some(Outcome::SpawnFailed {
        reason: SpawnReason::NotFound,
        os_code: Some(2),
    });
    for job in [signaled, failed] {
        let expected = json!(job.outcome);
        let peer = Peer::result(found(job));
        assert_eq!(
            App::new(peer.bridge())
                .invoke("job_by_id", json!({"job_id":JOB}))
                .unwrap()["data"]["job"]["outcome"],
            expected
        );
    }
}
