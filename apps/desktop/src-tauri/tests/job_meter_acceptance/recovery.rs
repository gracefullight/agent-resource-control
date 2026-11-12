use super::{daemon_bridge_support::HeldJob, serialization, support::*};
use arc_core::process_identity::{probe_live, random_job_id, IdentityStatus, ProcessIdentity};
use arc_ipc::Request;
use serde_json::json;
use std::{fs, process::Command, time::Instant};

#[test]
fn real_lost_completion_report_acks_duplicate_conflict_and_identity_mismatch() {
    let _serial = serialization::serial();
    let mut case = setup("job-meter-ack");
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let id = random_job_id().unwrap();
    let classification = json!({"classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNRESOLVED_COMMAND","uncertainty":"OBSERVE"});
    // Use the same frozen lifecycle shape old protocol-1 peers understand.
    ok(
        &case,
        "job.start",
        json!({"action":"REGISTER","job_id":id,"observer":observer,"classification":classification}),
    );
    ok(
        &case,
        "job.start",
        json!({"action":"CLAIM","job_id":id,"observer":observer}),
    );
    let started = Instant::now();
    let mut child = Command::new(case.fixture.binary("arc-exec-fixture"))
        .env("ARC_FIXTURE_ACCOUNTING", "success")
        .spawn()
        .unwrap();
    let child_identity = ProcessIdentity::capture(child.id()).unwrap();
    ok(
        &case,
        "job.start",
        json!({"action":"ATTACH","job_id":id,"observer":observer,"child":child_identity}),
    );
    let mut raw_status = 0;
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    let reaped = unsafe { libc::wait4(child.id() as i32, &mut raw_status, 0, &mut usage) };
    assert_eq!(reaped, child.id() as i32);
    assert!(libc::WIFEXITED(raw_status));
    assert_eq!(libc::WEXITSTATUS(raw_status), 0);
    // std::Child has no destructor side effect. wait4 above is the sole reaper.
    let _ = child.try_wait();
    let elapsed = started.elapsed().as_secs_f64();
    let complete = json!({"job_id":id,"observer":observer,"outcome":{"kind":"EXITED","code":0}});
    let mut complete_socket = case.fixture.connect();
    let mut request = Request::new("lost-completion-ack", "job.complete");
    request.payload = complete.clone();
    arc_ipc::write_message(&mut complete_socket, &request).unwrap();
    // Read a different authoritative channel while the ACK remains unread.
    eventually(|| case.snapshot(&id).outcome.is_some());
    drop(complete_socket);
    let completion_retry = ok(&case, "job.complete", complete);
    let unavailable = summary(&case, &id);
    assert_eq!(
        unavailable["accounting"],
        json!({"state":"unknown","reason":"unavailable"})
    );
    let cvt = |v: libc::timeval| v.tv_sec as f64 + f64::from(v.tv_usec) / 1e6;
    let user = cvt(usage.ru_utime);
    let system = cvt(usage.ru_stime);
    let report = json!({"schema_version":1,"report_id":random_job_id().unwrap(),"job_id":id,
        "observer":observer,"child":child_identity,"cpu_core_seconds":{"user":user,"system":system,"total":user+system},
        "run_elapsed":{"seconds":elapsed,"clock":"monotonic","includes_stopped":true},
        "attribution_basis":"native_owned_child_wait4","scope":"native_waited_descendants","completeness":"partial",
        "missing_coverage":["unobserved_descendants"],"accounting_relation":{"kind":"canonical_root","root_job_id":id},
        "outcome":{"kind":"EXITED","code":0},"terminated_at_ms":now()});
    // Compare the JSON wire value the server receives, including its numeric
    // representation, rather than an unencoded floating-point addition.
    let report = serde_json::from_slice(&serde_json::to_vec(&report).unwrap()).unwrap();
    arc_ipc::meter::decode_report(&report).unwrap();
    let mut socket = case.fixture.connect();
    let mut request = Request::new("lost-report-ack", "meter.report");
    request.payload = report.clone();
    arc_ipc::write_message(&mut socket, &request).unwrap();
    eventually(|| summary(&case, &id)["accounting"]["state"] == "recorded");
    drop(socket);
    let duplicate = ok(&case, "meter.report", report.clone());
    assert_eq!(duplicate["disposition"], "unchanged");
    let mut conflict = report.clone();
    conflict["run_elapsed"]["seconds"] = json!(elapsed + 1.0);
    let conflict = rpc(&case, "meter.report", conflict);
    assert_eq!(
        conflict.error.as_ref().unwrap().code,
        arc_ipc::meter::ERROR_METER_REPORT_CONFLICT
    );
    let mut mismatch = report.clone();
    mismatch["child"]["start_token"] = json!("1");
    let mismatch = rpc(&case, "meter.report", mismatch);
    assert_eq!(
        mismatch.error.as_ref().unwrap().code,
        arc_ipc::meter::ERROR_METER_IDENTITY_MISMATCH
    );
    let mut forged_observer = report.clone();
    forged_observer["observer"]["start_token"] = json!("1");
    let forged_observer = rpc(&case, "meter.report", forged_observer);
    assert_eq!(
        forged_observer.error.as_ref().unwrap().code,
        arc_ipc::meter::ERROR_METER_IDENTITY_MISMATCH
    );
    let surfaces = reconcile(&case, &id);
    assert_eq!(&surfaces["sqliteReport"], &report);
    let before = case.identity();
    case.stop("ack-before-restart");
    let offline = case.bridge.daemon_health().unwrap_err();
    case.start();
    let after = case.identity();
    let health = case.bridge.daemon_health().unwrap();
    assert_ne!(before, after);
    let retried = ok(&case, "meter.report", report.clone());
    assert_eq!(retried["disposition"], "unchanged");
    assert_eq!(super::support::report(&summary(&case, &id)), &report);
    let row_count = rows(
        &case,
        "SELECT count(*) AS count FROM job_meter_summaries WHERE report_id IS NOT NULL",
    );
    assert_eq!(row_count[0]["count"], 1);
    let errors = fs::read_to_string(case.fixture.root.join("stderr")).unwrap();
    assert!(errors.is_empty());
    save(
        "real_lost_completion_report_acks_duplicate_conflict_and_identity_mismatch",
        json!({
        "jobId":id,"completionRetry":completion_retry,"report":report,"duplicate":duplicate,
        "conflict":conflict,"childMismatch":mismatch,"observerMismatch":forged_observer,
        "withoutReport":unavailable,"surfaces":surfaces,"rawWait4":{"userSeconds":usage.ru_utime.tv_sec,
            "userMicroseconds":usage.ru_utime.tv_usec,"systemSeconds":usage.ru_stime.tv_sec,"systemMicroseconds":usage.ru_stime.tv_usec},
        "restart":{"before":before,"after":after,"offlineHealth":offline,"health":health,"recentStderr":errors,"retry":retried},
        "ackLoss":"completion and report ACKs were never read; independent status/summary confirmed commit before socket closure",
        "acceptedRows":1,"singleNativeExecution":true}),
    );
    case.stop("ack-done");
}

#[test]
fn real_daemon_restart_during_execution_and_interrupted_observer_remain_truthful() {
    let _serial = serialization::serial();
    let mut case = setup("job-meter-interrupted");
    let mut active = HeldJob::launch(&case, 17);
    let running = case.wait_running(&active.identity);
    let id = super::daemon_bridge_support::detail(&running)
        .job_id
        .as_str()
        .to_owned();
    let before = case.identity();
    case.stop("active-before-restart");
    assert_eq!(probe_live(&active.identity), IdentityStatus::VerifiedMatch);
    assert_eq!(
        probe_live(&active.wrapper_identity),
        IdentityStatus::VerifiedMatch
    );
    let offline = case.bridge.meter_summary(&id).unwrap_err();
    case.start();
    let health = case.bridge.daemon_health().unwrap();
    let after = case.identity();
    assert_ne!(before, after);
    let not_terminated = summary(&case, &id);
    assert_eq!(
        not_terminated["accounting"],
        json!({"state":"unknown","reason":"not_terminated"})
    );
    assert_eq!(active.release_and_wait().code(), Some(17));
    let survived = reconcile(&case, &id);
    drop(active);
    fs::remove_file(case.fixture.root.join("child.sock")).unwrap();
    let mut interrupted = HeldJob::launch(&case, 17);
    let running = case.wait_running(&interrupted.identity);
    let interrupted_id = super::daemon_bridge_support::detail(&running)
        .job_id
        .as_str()
        .to_owned();
    signal_owned(&interrupted.wrapper_identity, libc::SIGKILL);
    // Release the known bounded target before asserting observer interruption.
    let killed_status = interrupted.release_and_wait();
    assert!(!killed_status.success());
    eventually(|| probe_live(&interrupted.identity) == IdentityStatus::Missing);
    case.bridge.job_by_id(&interrupted_id).unwrap(); // native liveness reconciliation
    let unknown = summary(&case, &interrupted_id);
    assert_eq!(
        unknown["accounting"],
        json!({"state":"unknown","reason":"interrupted"})
    );
    let errors = fs::read_to_string(case.fixture.root.join("stderr")).unwrap();
    assert!(errors.is_empty());
    save(
        "real_daemon_restart_during_execution_and_interrupted_observer_remain_truthful",
        json!({
        "before":before,"after":after,"offlineSummary":offline,"health":health,"recentStderr":errors,
        "runningSummary":not_terminated,"survived":survived,"interruptedSummary":unknown,
        "interruptedObserver":interrupted.wrapper_identity,"interruptedChild":interrupted.identity,
        "interruptedChildFinallyAbsent":true,"launchCount":2}),
    );
    case.stop("interrupted-done");
}

#[test]
fn real_protocol_one_old_daemon_keeps_summary_unsupported_and_execution_once() {
    let _serial = serialization::serial();
    let legacy: serde_json::Value = serde_json::from_slice(
        &fs::read(workspace().join("target/host-meter/legacy/latest.json"))
            .expect("prepare accepted legacy peer with bun scripts/host-meter/prepare-legacy.ts"),
    )
    .unwrap();
    assert_eq!(
        legacy["acceptedHead"],
        "efedf0f4eb9b97eed641c82f3c4766bb16f2e3de"
    );
    let mut case = super::daemon_bridge_support::RealDaemon::meter_process(
        "job-meter-old-peer",
        workspace().join(legacy["binary"]["path"].as_str().unwrap()),
    );
    let cli = super::daemon_bridge_support::binary("arcctl");
    let marker = case.fixture.root.join("old-native-launches");
    let mut command = Command::new(&cli);
    command
        .args(["exec", "--"])
        .arg(case.fixture.binary("arc-exec-fixture"))
        .env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket)
        .env("ARC_FIXTURE_MODE", "success")
        .env("ARC_FIXTURE_MARKER", &marker)
        .env_remove("ARC_FIXTURE_ACCOUNTING")
        .env_remove("ARC_TRAFFIC_LINEAGE");
    let mut run = Run::launch(&case, command);
    assert!(run.finish().success(), "{}", run.errors());
    let id = job_id(&case, run.identity.pid);
    let result = Command::new(&cli)
        .args(["meter", "summary", "--job", &id, "--json"])
        .env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(stderr.starts_with("UNSUPPORTED_CAPABILITY"));
    let router = super::meter_router::Router::new(case.bridge.clone());
    let error = router
        .call("meter_summary", json!({"job_id":id}))
        .unwrap_err();
    assert_eq!(error["code"], "unsupported");
    let health = router.call("daemon_health", json!({})).unwrap();
    assert_eq!(health["data"]["protocol_version"], 1);
    assert_eq!(
        case.snapshot(&id).outcome,
        Some(arc_ipc::jobs::Outcome::Exited { code: 0 })
    );
    assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
    assert_eq!(
        rows(&case, "SELECT count(*) AS count FROM job_lifecycle")[0]["count"],
        1
    );
    save(
        "real_protocol_one_old_daemon_keeps_summary_unsupported_and_execution_once",
        json!({
        "legacyBuild":legacy,"jobId":id,"cliExit":result.status.code(),"cliStdoutEmpty":true,"cliStderr":stderr,
        "desktopError":error,"health":health,"executionExit":0,"singleExecutionCount":1,
        "frozenLifecycleSnapshot":case.snapshot(&id)}),
    );
    case.stop("old-peer-done");
}
