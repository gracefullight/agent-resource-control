//! Controlled clock against a test-owned SQLite database containing native
//! child measurements. No clock override or TTL is exposed by production IPC.
#![cfg(target_os = "macos")]
use arc_core::{job_meter::*, process_identity::ProcessIdentity};
use arc_ipc::jobs::{CompleteRequest, JobId, Outcome, StartRequest};
use arc_storage::{
    job_meter::{self, LineageEvidence},
    jobs::{self, StartEvidence},
};
use serde_json::json;
use std::{fs, os::unix::fs::DirBuilderExt, process::Command, time::Instant};

fn incomplete(_: &ProcessIdentity) -> (Vec<ProcessIdentity>, bool) {
    (vec![], false)
}
fn owned(child: &ProcessIdentity, observer: &ProcessIdentity) -> bool {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&info) as i32;
    (unsafe {
        libc::proc_pidinfo(
            child.pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    }) == size
        && info.pbi_ppid == observer.pid
        && ProcessIdentity::capture(child.pid).is_ok_and(|i| &i == child)
}
struct NativeChild(std::process::Child, ProcessIdentity);
impl Drop for NativeChild {
    fn drop(&mut self) {
        if arc_core::process_identity::probe_live(&self.1)
            == arc_core::process_identity::IdentityStatus::VerifiedMatch
        {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
#[test]
fn real_native_measurement_obeys_ninety_day_boundary_without_resurrection() {
    let id = arc_core::process_identity::random_job_id().unwrap();
    let root =
        std::env::temp_dir().join(format!("arc-native-retention-{}-{id}", std::process::id()));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let mut c = arc_storage::open_and_migrate(root.join("fixture.sqlite3")).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let job_id = JobId::try_from(id.clone()).unwrap();
    let class = serde_json::from_value(json!({"classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNRESOLVED_COMMAND","uncertainty":"OBSERVE"})).unwrap();
    for request in [
        StartRequest::Register {
            job_id: job_id.clone(),
            observer: observer.clone(),
            classification: class,
        },
        StartRequest::Claim {
            job_id: job_id.clone(),
            observer: observer.clone(),
        },
    ] {
        jobs::start(&mut c, &request, &StartEvidence::gather(&request), 100).unwrap();
    }
    let started = Instant::now();
    let child = Command::new("/bin/sleep").arg("0.15").spawn().unwrap();
    let identity = ProcessIdentity::capture(child.id()).unwrap();
    let mut child = NativeChild(child, identity.clone());
    let attach = StartRequest::Attach {
        job_id: job_id.clone(),
        observer: observer.clone(),
        child: identity.clone(),
    };
    let evidence = StartEvidence::gather(&attach).with_meter_evidence(LineageEvidence::gather(
        &observer,
        Some(&identity),
        incomplete,
        owned,
    ));
    jobs::start(&mut c, &attach, &evidence, 110).unwrap();
    let mut status = 0;
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::wait4(child.0.id() as i32, &mut status, 0, &mut usage) },
        child.0.id() as i32
    );
    let _ = child.0.try_wait();
    assert!(libc::WIFEXITED(status));
    assert_eq!(libc::WEXITSTATUS(status), 0);
    jobs::complete(
        &mut c,
        &CompleteRequest {
            job_id: job_id.clone(),
            observer: observer.clone(),
            outcome: Outcome::Exited { code: 0 },
        },
        200,
    )
    .unwrap();
    let cvt = |v: libc::timeval| v.tv_sec as f64 + f64::from(v.tv_usec) / 1e6;
    let user = cvt(usage.ru_utime);
    let system = cvt(usage.ru_stime);
    let report:JobMeterReport = serde_json::from_value(json!({"schema_version":1,"report_id":arc_core::process_identity::random_job_id().unwrap(),
        "job_id":id,"observer":observer,"child":identity,"cpu_core_seconds":{"user":user,"system":system,"total":user+system},
        "run_elapsed":{"seconds":started.elapsed().as_secs_f64(),"clock":"monotonic","includes_stopped":true},
        "attribution_basis":"native_owned_child_wait4","scope":"native_waited_descendants","completeness":"partial",
        "missing_coverage":["unobserved_descendants","lineage_unverified"],"accounting_relation":{"kind":"unverified"},
        "outcome":{"kind":"EXITED","code":0},"terminated_at_ms":200})).unwrap();
    job_meter::report(&mut c, &report, 300).unwrap();
    let boundary = 200 + JOB_METADATA_RETENTION_MS;
    assert_eq!(job_meter::expire(&mut c, boundary - 1).unwrap(), 0);
    let before = job_meter::summary(&mut c, &id, boundary - 1).unwrap();
    assert!(matches!(
        before.accounting,
        SummaryAccounting::Recorded { .. }
    ));
    assert_eq!(job_meter::expire(&mut c, boundary).unwrap(), 1);
    let after = job_meter::summary(&mut c, &id, boundary).unwrap();
    assert_eq!(after.accounting, SummaryAccounting::Expired {});
    assert_eq!(after.queue_wait, QueueWait::unknown());
    assert_eq!(
        job_meter::report(&mut c, &report, boundary),
        Err(job_meter::MeterError::Unavailable)
    );
    assert_eq!(
        job_meter::summary(&mut c, &id, 300).unwrap().accounting,
        SummaryAccounting::Expired {}
    );
    let remaining:(i64,i64,i64) = c.query_row("SELECT (SELECT count(*) FROM jobs),(SELECT count(*) FROM job_lifecycle),(SELECT count(*) FROM job_meter_summaries WHERE report_json IS NOT NULL)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(remaining, (1, 1, 0));
    c.close().unwrap();
    // Restore the pre-meter schema in a second private fixture, retaining the
    // real native job/lifecycle rows. This explicitly models a v3 upgrade;
    // it does not label a new job's missing report as legacy.
    let legacy_path = root.join("restored-v3.sqlite3");
    fs::copy(root.join("fixture.sqlite3"), &legacy_path).unwrap();
    let legacy = rusqlite::Connection::open(&legacy_path).unwrap();
    legacy
        .execute_batch(
            "DROP INDEX idx_policy_decisions_job_time;
        DROP INDEX idx_policy_decisions_expiry;
        ALTER TABLE policy_states DROP COLUMN policy_version;
        ALTER TABLE policy_states DROP COLUMN state_json;
        ALTER TABLE policy_decisions DROP COLUMN observation_availability;
        ALTER TABLE policy_decisions DROP COLUMN observation_age_ms;
        ALTER TABLE policy_decisions DROP COLUMN observation_json;
        ALTER TABLE policy_decisions DROP COLUMN transition_key;
        ALTER TABLE policy_decisions DROP COLUMN permit_generation;
        ALTER TABLE policy_decisions DROP COLUMN recorded_at_ms;
        ALTER TABLE policy_decisions DROP COLUMN expires_at_ms;
        DROP TABLE job_meter_summaries;
        DROP INDEX idx_lifecycle_observer_identity; DROP INDEX idx_lifecycle_child_identity;
        DELETE FROM schema_migrations WHERE id IN ('0004_job_meter','0005_memory_pressure');
        PRAGMA user_version=3;",
        )
        .unwrap();
    legacy.close().unwrap();
    let mut upgraded = arc_storage::open_and_migrate(&legacy_path).unwrap();
    let legacy_summary = job_meter::summary(&mut upgraded, &id, 300).unwrap();
    assert_eq!(
        legacy_summary.accounting,
        SummaryAccounting::Unknown {
            reason: UnknownReason::Legacy
        }
    );
    assert_eq!(legacy_summary.queue_wait, QueueWait::unknown());
    let schema = arc_storage::assess_schema(&upgraded).unwrap();
    assert!(schema.history_verified);
    assert_eq!(schema.applied_version, Some(5));
    upgraded.close().unwrap();
    let observed = json!({"test":"real_native_measurement_obeys_ninety_day_boundary_without_resurrection",
        "controlledClock":{"terminalAtMs":200,"retentionMs":JOB_METADATA_RETENTION_MS,"beforeMs":boundary-1,"boundaryMs":boundary},
        "rawWait4":{"userSeconds":usage.ru_utime.tv_sec,"userMicroseconds":usage.ru_utime.tv_usec,
        "systemSeconds":usage.ru_stime.tv_sec,"systemMicroseconds":usage.ru_stime.tv_usec},
        "before":before,"after":after,"remainingJobRows":remaining.0,"remainingLifecycleRows":remaining.1,
        "remainingPayloads":remaining.2,"lateDeliveryRejected":true,"backwardClockCannotResurrect":true,
        "legacyUpgrade":{"summary":legacy_summary,"nativeFactsPreserved":true,"historyVerified":schema.history_verified,
            "fixtureMethod":"copy native fixture after close; restore schema v3 objects, history and user_version witness; reopen through production migrator"},
        "testOwnedDatabase":root.join("fixture.sqlite3")});
    if let Some(dir) = std::env::var_os("ARC_JOB_METER_EVIDENCE_DIR") {
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            std::path::Path::new(&dir).join(
                "real_native_measurement_obeys_ninety_day_boundary_without_resurrection.json",
            ),
            serde_json::to_vec_pretty(&observed).unwrap(),
        )
        .unwrap();
    }
    eprintln!("native-retention {observed}");
    fs::remove_dir_all(root).unwrap();
}
