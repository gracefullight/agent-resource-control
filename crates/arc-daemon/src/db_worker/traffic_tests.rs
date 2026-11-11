use super::*;
use crate::{
    db_worker::{DbConfig, DbWorker},
    limits::RuntimeLimits,
    runtime_metrics::RuntimeMetrics,
};
use arc_ipc::traffic::JobResult;
use std::{num::NonZeroUsize, process::Command, sync::Arc, time::Duration};

struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn attachment_between_snapshot_and_transaction_refreshes_lineage_proof() {
    let root = arc_core::runtime_root().unwrap().join(format!(
        "arc-lineage-race-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let path = root.join("arc.sqlite3");
    let connection = arc_storage::open_and_migrate(&path).unwrap();
    let limits = RuntimeLimits::default();
    let mut worker = DbWorker::start(
        connection,
        DbConfig {
            queue_capacity: NonZeroUsize::new(8).unwrap(),
            busy_timeout: Duration::from_millis(30),
            execution_timeout: Duration::from_secs(1),
        },
        Arc::new(RuntimeMetrics::new(&limits)),
    )
    .unwrap();
    let client = worker.client();
    let owner = ProcessIdentity::capture(std::process::id()).unwrap();
    let child = Child(Command::new("/bin/sleep").arg("30").spawn().unwrap());
    let nested_owner = ProcessIdentity::capture(child.0.id()).unwrap();
    assert!(is_descendant(&nested_owner, &owner));
    let classification = serde_json::from_value(serde_json::json!({"classification":"BUILD","locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"})).unwrap();
    let root_id = JobId::try_from("a123456789abcdef0123456789abcdef".to_owned()).unwrap();
    let root_request = JobRequest::Enqueue {
        job_id: root_id.clone(),
        observer: owner.clone(),
        classification,
        lineage_context: None,
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    let root_reply: JobResult = serde_json::from_value(
        handle(client.clone(), root_request, owner.pid, deadline)
            .await
            .unwrap(),
    )
    .unwrap();
    let JobResult::Granted {
        permit_generation,
        lineage_context,
        ..
    } = root_reply
    else {
        panic!("expected root grant")
    };
    let nested = JobRequest::Enqueue {
        job_id: JobId::try_from("b123456789abcdef0123456789abcdef".to_owned()).unwrap(),
        observer: nested_owner.clone(),
        classification,
        lineage_context: Some(lineage_context),
    };
    let attachment = JobRequest::Attach {
        job_id: root_id,
        observer: owner,
        child: nested_owner.clone(),
        permit_generation,
    };
    let mut other = arc_storage::open_and_migrate(&path).unwrap();
    let mut raced = false;
    let mut proofs = 0;
    let result = handle_inner(
        client.clone(),
        nested.clone(),
        nested_owner.pid,
        deadline,
        |has_proof| {
            if has_proof {
                proofs += 1;
                if !raced {
                    let evidence = traffic::ChildEvidence::gather(&nested_owner);
                    assert!(matches!(
                        traffic::attach(&mut other, &attachment, &evidence, jobs::now()).unwrap(),
                        JobResult::Running {}
                    ));
                    raced = true;
                }
            }
        },
    )
    .await
    .unwrap();
    let result: JobResult = serde_json::from_value(result).unwrap();
    assert!(
        raced && proofs >= 2,
        "attachment must invalidate and refresh the proof"
    );
    assert!(matches!(result, JobResult::Reused { .. }));
    let duplicate: JobResult = serde_json::from_value(
        handle(client.clone(), nested, nested_owner.pid, deadline)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result, duplicate);
    assert_eq!(traffic::active_reservations(&other).unwrap().build, 1);
    let permit_count: i64 = other
        .query_row("SELECT count(*) FROM permits", [], |row| row.get(0))
        .unwrap();
    assert_eq!(permit_count, 1);
    drop(other);
    worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    drop(child);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sibling_claim_and_stale_identity_cannot_reuse_an_active_permit() {
    let root = arc_core::runtime_root().unwrap().join(format!(
        "arc-lineage-sibling-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let path = root.join("arc.sqlite3");
    let connection = arc_storage::open_and_migrate(&path).unwrap();
    let limits = RuntimeLimits::default();
    let mut worker = DbWorker::start(
        connection,
        DbConfig {
            queue_capacity: NonZeroUsize::new(8).unwrap(),
            busy_timeout: Duration::from_millis(30),
            execution_timeout: Duration::from_secs(1),
        },
        Arc::new(RuntimeMetrics::new(&limits)),
    )
    .unwrap();
    let client = worker.client();
    let left = Child(Command::new("/bin/sleep").arg("30").spawn().unwrap());
    let right = Child(Command::new("/bin/sleep").arg("30").spawn().unwrap());
    let left_id = ProcessIdentity::capture(left.0.id()).unwrap();
    let right_id = ProcessIdentity::capture(right.0.id()).unwrap();
    assert!(!is_descendant(&right_id, &left_id));
    let classification = serde_json::from_value(serde_json::json!({"classification":"TEST","locality":"HOST_LOCAL","reason":"CARGO_TEST","uncertainty":"NONE"})).unwrap();
    let root_id = JobId::try_from("a223456789abcdef0123456789abcdef".to_owned()).unwrap();
    let nested_id = JobId::try_from("b223456789abcdef0123456789abcdef".to_owned()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    // handle receives the same peer PID the socket adapter would authenticate.
    let granted: JobResult = serde_json::from_value(
        handle(
            client.clone(),
            JobRequest::Enqueue {
                job_id: root_id,
                observer: left_id.clone(),
                classification,
                lineage_context: None,
            },
            left_id.pid,
            deadline,
        )
        .await
        .unwrap(),
    )
    .unwrap();
    let JobResult::Granted {
        lineage_context, ..
    } = granted
    else {
        panic!("expected grant")
    };
    let forged = JobRequest::Enqueue {
        job_id: nested_id,
        observer: right_id.clone(),
        classification,
        lineage_context: Some(lineage_context),
    };
    let reply: JobResult = serde_json::from_value(
        handle(client.clone(), forged.clone(), right_id.pid, deadline)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(reply, JobResult::Queued { .. }));
    let mut stale = right_id.clone();
    stale.start_token = (stale.start_token.parse::<u64>().unwrap() + 1).to_string();
    let stale_request = JobRequest::Poll {
        job_id: JobId::try_from("b223456789abcdef0123456789abcdef".to_owned()).unwrap(),
        observer: stale,
    };
    assert_eq!(
        handle(client.clone(), stale_request, right_id.pid, deadline).await,
        Err(arc_ipc::ERROR_INVALID_REQUEST)
    );
    assert_eq!(
        handle(client.clone(), forged, left_id.pid, deadline).await,
        Err(arc_ipc::ERROR_INVALID_REQUEST)
    );
    let db = arc_storage::open_and_migrate(&path).unwrap();
    let counts: (i64, i64) = db.query_row("SELECT (SELECT count(*) FROM permits WHERE state='ACTIVE'), (SELECT count(*) FROM traffic_entries WHERE root_job_id IS NOT NULL)", [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!(counts, (1, 0));
    drop(db);
    worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    drop((left, right));
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn released_permit_between_ancestry_proof_and_commit_is_not_reused() {
    let root = arc_core::runtime_root().unwrap().join(format!(
        "arc-lineage-release-race-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let path = root.join("arc.sqlite3");
    let connection = arc_storage::open_and_migrate(&path).unwrap();
    let limits = RuntimeLimits::default();
    let mut worker = DbWorker::start(
        connection,
        DbConfig {
            queue_capacity: NonZeroUsize::new(8).unwrap(),
            busy_timeout: Duration::from_millis(30),
            execution_timeout: Duration::from_secs(1),
        },
        Arc::new(RuntimeMetrics::new(&limits)),
    )
    .unwrap();
    let client = worker.client();
    let owner = ProcessIdentity::capture(std::process::id()).unwrap();
    let child = Child(Command::new("/bin/sleep").arg("30").spawn().unwrap());
    let nested_owner = ProcessIdentity::capture(child.0.id()).unwrap();
    let classification = serde_json::from_value(serde_json::json!({"classification":"TEST","locality":"HOST_LOCAL","reason":"CARGO_TEST","uncertainty":"NONE"})).unwrap();
    let root_id = JobId::try_from("a323456789abcdef0123456789abcdef".to_owned()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let grant: JobResult = serde_json::from_value(
        handle(
            client.clone(),
            JobRequest::Enqueue {
                job_id: root_id.clone(),
                observer: owner.clone(),
                classification,
                lineage_context: None,
            },
            owner.pid,
            deadline,
        )
        .await
        .unwrap(),
    )
    .unwrap();
    let JobResult::Granted {
        permit_generation, ..
    } = grant
    else {
        panic!("expected grant")
    };
    let mut other = arc_storage::open_and_migrate(&path).unwrap();
    let mut released = false;
    let request = JobRequest::Enqueue {
        job_id: JobId::try_from("b323456789abcdef0123456789abcdef".to_owned()).unwrap(),
        observer: nested_owner.clone(),
        classification,
        lineage_context: None,
    };
    let reply: JobResult = serde_json::from_value(
        handle_inner(client, request, nested_owner.pid, deadline, |has_proof| {
            if has_proof && !released {
                let complete = JobRequest::Complete {
                    job_id: root_id.clone(),
                    observer: owner.clone(),
                    permit_generation,
                    outcome: arc_ipc::jobs::Outcome::Exited { code: 0 },
                };
                assert!(matches!(
                    traffic::complete(&mut other, &complete, jobs::now()).unwrap(),
                    JobResult::Completed { .. }
                ));
                released = true;
            }
        })
        .await
        .unwrap(),
    )
    .unwrap();
    assert!(released);
    assert!(
        matches!(reply, JobResult::Granted { .. }),
        "stale proof must not reuse released permit: {reply:?}"
    );
    assert_eq!(traffic::active_reservations(&other).unwrap().test, 1);
    let counts: (i64, i64) = other
        .query_row(
            "SELECT count(*),sum(state='ACTIVE') FROM permits",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(counts, (2, 1));
    drop(other);
    worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    drop(child);
    std::fs::remove_dir_all(root).unwrap();
}
