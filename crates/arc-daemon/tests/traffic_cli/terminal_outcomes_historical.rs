use super::terminal_outcomes::{assert_state, restart_daemon, status, stored_record};
use super::*;
use arc_ipc::jobs::{JobId, StatusRequest, StatusResult, OPERATION_JOB_STATUS};
use arc_ipc::traffic::JobRequest;

#[test]
fn historical_lifecycle_outcomes_are_labeled_only_when_known() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let mut ids = Vec::new();
    for (index, mode) in ["success", "echo", "echo", "echo"].iter().enumerate() {
        let output = Command::new(binary("arcctl"))
            .arg("exec")
            .arg("--")
            .arg(binary("arc-exec-fixture"))
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env("ARC_FIXTURE_MODE", mode)
            .env(
                "ARC_FIXTURE_MARKER",
                f.root.join(format!("historical-{index}")),
            )
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(if index == 0 { 0 } else { 17 }));
        let id: String = database(&f)
            .query_row(
                "SELECT job_id FROM job_lifecycle ORDER BY rowid DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        ids.push(id);
    }
    daemon.stop(libc::SIGTERM);
    f.report();
    let db = database(&f);
    for id in &ids {
        db.execute("UPDATE jobs SET state='COMPLETED' WHERE id=?1", [id])
            .unwrap();
    }
    for id in &ids[2..] {
        db.execute("UPDATE jobs SET exit_code=NULL WHERE id=?1", [id])
            .unwrap();
    }
    db.execute(
        "UPDATE job_lifecycle SET outcome_kind=NULL WHERE job_id=?1",
        [&ids[2]],
    )
    .unwrap();
    drop(db);
    let mut restarted = restart_daemon(&f);
    for (id, expected_kind) in ids[2..].iter().zip([None, Some("EXITED")]) {
        let (finished, kind): (Option<i64>, Option<String>) = database(&f)
            .query_row(
                "SELECT j.finished_at,l.outcome_kind FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id WHERE j.id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(finished.is_some());
        assert_eq!(kind.as_deref(), expected_kind);
    }
    let recent = status(&f, None);
    for (id, expected) in
        ids.iter()
            .zip(["succeeded", "failed", "outcome unknown", "outcome unknown"])
    {
        assert_state(&recent, id, expected);
        let before = stored_record(&f, id);
        assert_state(&status(&f, Some(id)), id, expected);
        assert_eq!(
            stored_record(&f, id),
            before,
            "status changed stored job {id}"
        );
    }
    restarted.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn historical_unknown_preserves_verified_child_in_cli_and_protocol() {
    use arc_ipc::jobs::{CompleteRequest, Outcome, ProcessIdentity, StartRequest};
    let _serial = support::serial();
    let f = Fixture::new();
    let mut db = arc_storage::open_and_migrate(&f.database).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let classification = serde_json::from_value(serde_json::json!({
        "classification":"UNKNOWN", "locality":"UNKNOWN",
        "reason":"UNSUPPORTED_SYNTAX", "uncertainty":"OBSERVE"
    }))
    .unwrap();
    let ids: Vec<_> = (1..=2)
        .map(|n| JobId::try_from(format!("{n:032x}")).unwrap())
        .collect();
    for (index, id) in ids.iter().enumerate() {
        for request in [
            StartRequest::Register {
                job_id: id.clone(),
                observer: observer.clone(),
                classification,
            },
            StartRequest::Claim {
                job_id: id.clone(),
                observer: observer.clone(),
            },
            StartRequest::Attach {
                job_id: id.clone(),
                observer: observer.clone(),
                child: observer.clone(),
            },
        ] {
            let evidence = arc_storage::jobs::StartEvidence::gather(&request);
            arc_storage::jobs::start(&mut db, &request, &evidence, 100).unwrap();
        }
        arc_storage::jobs::complete(
            &mut db,
            &CompleteRequest {
                job_id: id.clone(),
                observer: observer.clone(),
                outcome: Outcome::Exited { code: 17 },
            },
            101,
        )
        .unwrap();
        db.execute(
            "UPDATE jobs SET state='COMPLETED', exit_code=NULL WHERE id=?1",
            [id.as_str()],
        )
        .unwrap();
        if index == 0 {
            db.execute(
                "UPDATE job_lifecycle SET outcome_kind=NULL WHERE job_id=?1",
                [id.as_str()],
            )
            .unwrap();
        }
    }
    drop(db);
    let before: Vec<_> = ids
        .iter()
        .map(|id| stored_record(&f, id.as_str()))
        .collect();
    let mut daemon = f.spawn("test");
    for (id, before) in ids.iter().zip(before) {
        let mut request = arc_ipc::Request::new("historical-child", OPERATION_JOB_STATUS);
        request.payload = serde_json::to_value(StatusRequest::ById { job_id: id.clone() }).unwrap();
        let response = arc_ipc::exchange(&mut f.connect(), &request).unwrap();
        assert_eq!(response.protocol_version, 1);
        let value = response.result.unwrap();
        let decoded: StatusResult = serde_json::from_value(value.clone()).unwrap();
        assert!(decoded.is_valid());
        assert_eq!(value["job"]["phase"], "OUTCOME_UNKNOWN");
        assert_eq!(value["job"]["outcome"], serde_json::Value::Null);
        assert_eq!(value["job"]["observation"]["target"], "CHILD");
        assert_eq!(value["job"]["observation"]["status"], "VERIFIED_MATCH");
        assert_eq!(value["job"]["reason"], "NONE");
        assert_eq!(value["job"]["observation"]["reason"], "NONE");
        let detail = status(&f, Some(id.as_str()));
        assert_state(&detail, id.as_str(), "outcome unknown");
        assert!(
            detail.contains("Identity: verified at observation\n"),
            "{detail}"
        );
        assert!(detail.contains("Reason: NONE\n"), "{detail}");
        assert!(!detail.contains("IDENTITY_UNAVAILABLE"), "{detail}");
        let recent = status(&f, None);
        assert_state(&recent, id.as_str(), "outcome unknown");
        let block = recent
            .split("Job: ")
            .find(|s| s.starts_with(id.as_str()))
            .unwrap();
        assert!(block.contains("Reason: NONE\n"), "{block}");
        assert_eq!(stored_record(&f, id.as_str()), before);
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn historical_scheduled_unknowns_do_not_hide_later_known_outcomes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    symlink(binary("arc-exec-fixture"), f.root.join("cargo")).unwrap();
    let mut ids = Vec::new();
    for (index, mode) in ["success", "echo", "echo", "echo"].iter().enumerate() {
        let marker = f.root.join(format!("scheduled-historical-{index}"));
        let output = Command::new(binary("arcctl"))
            .args(["exec", "--", "cargo", "build"])
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env("PATH", &f.root)
            .env("ARC_FIXTURE_MODE", mode)
            .env("ARC_FIXTURE_MARKER", &marker)
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(if index == 0 { 0 } else { 17 }));
        assert_eq!(output.stderr, b"fixture-stderr\n");
        assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
        let id: String = database(&f)
            .query_row(
                "SELECT job_id FROM job_lifecycle ORDER BY rowid DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        ids.push(id);
    }
    assert_eq!(active(&f).1, 0);
    daemon.stop(libc::SIGTERM);
    f.report();
    let db = database(&f);
    for (index, id) in ids.iter().enumerate() {
        // Make recent ordering independent of clock resolution and random IDs.
        let registered = 100 + index as i64;
        db.execute(
            "UPDATE jobs SET state='COMPLETED',created_at=?2 WHERE id=?1",
            (id, registered),
        )
        .unwrap();
        db.execute(
            "UPDATE job_lifecycle SET registered_at_ms=?2 WHERE job_id=?1",
            (id, registered),
        )
        .unwrap();
    }
    for id in &ids[2..] {
        db.execute("UPDATE jobs SET exit_code=NULL WHERE id=?1", [id])
            .unwrap();
    }
    db.execute(
        "UPDATE job_lifecycle SET outcome_kind=NULL WHERE job_id=?1",
        [&ids[2]],
    )
    .unwrap();
    drop(db);
    let before: Vec<_> = ids[2..].iter().map(|id| stored_record(&f, id)).collect();
    let mut restarted = restart_daemon(&f);
    for id in &ids[2..] {
        let job_id = JobId::try_from(id.clone()).unwrap();
        // Neither closed protocol-1 Traffic status shape can represent a
        // completed entry without an outcome; clients may use job.status.
        for request in [
            JobRequest::Status {
                job_id: job_id.clone(),
            },
            JobRequest::StatusV2 {
                job_id: job_id.clone(),
            },
        ] {
            let response = traffic(&f, &request);
            assert_eq!(response.protocol_version, 1);
            assert_eq!(response.status, arc_ipc::STATUS_ERROR);
            assert_eq!(
                response.error.unwrap().code,
                arc_ipc::ERROR_UNSUPPORTED_CAPABILITY
            );
        }
        let detail = status(&f, Some(id));
        assert_state(&detail, id, "outcome unknown");
        assert!(!detail.contains("Exit code:"), "{detail}");
        assert!(!detail.contains("Reservation: retained"), "{detail}");
    }
    let recent = status(&f, None);
    assert!(recent.find(&ids[3]).unwrap() < recent.find(&ids[2]).unwrap());
    assert!(recent.find(&ids[2]).unwrap() < recent.find(&ids[1]).unwrap());
    assert!(recent.find(&ids[1]).unwrap() < recent.find(&ids[0]).unwrap());
    for (id, expected) in
        ids.iter()
            .zip(["succeeded", "failed", "outcome unknown", "outcome unknown"])
    {
        assert_state(&recent, id, expected);
        assert_state(&status(&f, Some(id)), id, expected);
    }
    for (id, original) in ids[2..].iter().zip(before) {
        assert_eq!(stored_record(&f, id), original, "status changed {id}");
    }
    restarted.stop(libc::SIGTERM);
    f.report();
}
