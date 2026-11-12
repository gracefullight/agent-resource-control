use super::{serialization, support::*};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use serde_json::json;
use std::{
    fs,
    os::unix::process::ExitStatusExt,
    thread,
    time::{Duration, Instant},
};

#[test]
fn real_scheduled_lifecycle_exit_signal_and_partial_summaries() {
    let _serial = serialization::serial();
    let mut case = setup("job-meter-lifecycle");
    let mut observations = vec![];
    for scheduled in [false, true] {
        for (mode, code) in [("success", Some(0)), ("work", Some(17)), ("signal", None)] {
            let mut run = Run::launch(&case, command(&case, scheduled, mode));
            let status = run.finish();
            assert_eq!(status.code(), code, "{}", run.errors());
            if code.is_none() {
                assert_eq!(status.signal(), Some(libc::SIGTERM));
            }
            assert_eq!(run.count(), 1);
            let id = job_id(&case, run.identity.pid);
            let surfaces = reconcile(&case, &id);
            let report = report(&surfaces["cli"]);
            assert_eq!(report["observer"], json!(run.identity));
            assert_eq!(report["completeness"], "partial");
            assert_eq!(report["scope"], "native_waited_descendants");
            assert_eq!(report["accounting_relation"]["kind"], "canonical_root");
            assert_eq!(report["run_elapsed"]["clock"], "monotonic");
            assert_eq!(report["run_elapsed"]["includes_stopped"], true);
            if code.is_some() {
                assert!(report["cpu_core_seconds"]["total"].as_f64().unwrap() > 0.01);
            }
            observations.push(json!({"scheduled":scheduled,"mode":mode,"nativeCode":code,"nativeSignal":status.signal(),
                "singleExecutionCount":run.count(),"surfaces":surfaces,"stderr":run.errors()}));
        }
    }
    save(
        "real_scheduled_lifecycle_exit_signal_and_partial_summaries",
        json!({"runs":observations}),
    );
    case.stop("lifecycle-done");
}

#[test]
fn real_queue_wait_is_separate_from_monotonic_running_duration() {
    let _serial = serialization::serial();
    let mut case = setup("job-meter-queue");
    let release = case.fixture.root.join("release");
    let ready = case.fixture.root.join("ready");
    let mut first_command = command(&case, true, "hold");
    first_command
        .env("ARC_METER_TEST_RELEASE", &release)
        .env("ARC_METER_TEST_READY", &ready);
    let mut first = Run::launch(&case, first_command);
    eventually(|| {
        ready.exists()
            && rows(&case, "SELECT state FROM traffic_entries")
                .iter()
                .any(|r| r["state"] == "RUNNING")
    });
    let started = Instant::now();
    let mut queued = Run::launch(&case, command(&case, true, "work"));
    eventually(|| {
        rows(&case, "SELECT t.state,l.observer_pid FROM traffic_entries t JOIN job_lifecycle l ON l.job_id=t.job_id")
        .iter().any(|r|r["observer_pid"]==queued.identity.pid && r["state"]=="QUEUED")
    });
    assert!(!queued.marker.exists(), "queued work must not start");
    thread::sleep(Duration::from_millis(250));
    fs::write(&release, b"release").unwrap();
    assert_eq!(first.finish().code(), Some(17));
    assert_eq!(queued.finish().code(), Some(17));
    let elapsed = started.elapsed().as_secs_f64();
    let id = job_id(&case, queued.identity.pid);
    let surfaces = reconcile(&case, &id);
    let summary = &surfaces["cli"];
    let queue = summary["queue_wait"]["seconds"].as_f64().unwrap();
    let run = report(summary)["run_elapsed"]["seconds"].as_f64().unwrap();
    assert!(queue >= 0.25, "{summary}");
    assert!(
        run >= 0.12 && run < elapsed - 0.2,
        "run={run} elapsed={elapsed}"
    );
    assert_eq!(summary["queue_wait"]["basis"], "queued_to_reserved");
    assert_eq!(summary["queue_wait"]["clock"], "daemon_wall_clock");
    assert_eq!(queued.count(), 1);
    assert_eq!(first.count(), 1);
    let raw = rows(
        &case,
        "SELECT job_id,queued_at_ms,granted_at_ms FROM traffic_entries",
    );
    let row = raw.iter().find(|r| r["job_id"] == id).unwrap();
    assert!(
        (queue
            - (row["granted_at_ms"].as_f64().unwrap() - row["queued_at_ms"].as_f64().unwrap())
                / 1000.0)
            .abs()
            < 1e-9
    );
    save(
        "real_queue_wait_is_separate_from_monotonic_running_duration",
        json!({"surfaces":surfaces,
        "rawQueueTimestamps":raw,"outerElapsedSeconds":elapsed,"queueSeconds":queue,"runSeconds":run,
        "queuePreventedSpawn":true,"singleExecutionCount":queued.count(),"stderr":queued.errors()}),
    );
    case.stop("queue-done");
}

#[test]
fn real_nested_modes_use_one_authoritative_accounting_root() {
    let _serial = serialization::serial();
    let mut case = setup("job-meter-nested");
    let mut observations = vec![];
    for scheduled in [false, true] {
        for mode in ["nested", "nested-scheduled"] {
            let mut run = Run::launch(&case, command(&case, scheduled, mode));
            assert_eq!(run.finish().code(), Some(17), "{}", run.errors());
            assert_eq!(run.count(), 2, "one outer child and one nested child");
            let id = job_id(&case, run.identity.pid);
            let root = summary(&case, &id);
            let root_report = report(&root);
            assert_eq!(
                root_report["accounting_relation"],
                json!({"kind":"canonical_root","root_job_id":id})
            );
            let nested_rows = rows(&case, "SELECT job_id,root_job_id,report_json FROM job_meter_summaries WHERE relation='included_in_root'");
            let nested = nested_rows.iter().find(|r| r["root_job_id"] == id).unwrap();
            let nested = summary(&case, nested["job_id"].as_str().unwrap());
            let nr = report(&nested);
            assert_eq!(nr["accounting_relation"]["kind"], "included_in_root");
            assert_eq!(nr["accounting_relation"]["root_job_id"], id);
            let authoritative = root_report["cpu_core_seconds"]["total"].as_f64().unwrap();
            let inclusive_nested = nr["cpu_core_seconds"]["total"].as_f64().unwrap();
            assert!(inclusive_nested > 0.01);
            assert!(authoritative + 0.001 >= inclusive_nested);
            observations.push(json!({"scheduledOuter":scheduled,"nestedMode":mode,"root":root,"nested":nested,
                "authoritativeCoreSeconds":authoritative,"nonAdditiveNestedCoreSeconds":inclusive_nested,
                "incorrectOverlappingSum":authoritative+inclusive_nested,"singleExecutionCount":run.count()}));
        }
    }
    save(
        "real_nested_modes_use_one_authoritative_accounting_root",
        json!({"runs":observations}),
    );
    case.stop("nested-done");
}

#[test]
fn real_stop_continue_and_escaped_descendant_keep_explicit_partial_scope() {
    let _serial = serialization::serial();
    let mut case = setup("job-meter-stopped");
    let mut run = Run::launch(&case, command(&case, false, "stop"));
    eventually(|| {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    run.identity.pid,
                    &mut info,
                    libc::WSTOPPED | libc::WNOHANG | libc::WNOWAIT,
                )
            },
            0
        );
        info.si_code == libc::CLD_STOPPED
    });
    let id = job_id(&case, run.identity.pid);
    let during = summary(&case, &id);
    assert_eq!(
        during["accounting"],
        json!({"state":"unknown","reason":"not_terminated"})
    );
    thread::sleep(Duration::from_millis(250));
    signal_owned(&run.identity, libc::SIGCONT);
    assert_eq!(run.finish().code(), Some(17));
    let stopped = summary(&case, &id);
    assert!(report(&stopped)["run_elapsed"]["seconds"].as_f64().unwrap() >= 0.37);
    assert_eq!(report(&stopped)["run_elapsed"]["includes_stopped"], true);
    let escaped_path = case.fixture.root.join("escaped.json");
    let mut c = command(&case, false, "escaped");
    c.env("ARC_METER_TEST_ESCAPED", &escaped_path);
    let mut escaped = Run::launch(&case, c);
    assert!(escaped.finish().success());
    let descendant: ProcessIdentity =
        serde_json::from_slice(&fs::read(&escaped_path).unwrap()).unwrap();
    assert_eq!(probe_live(&descendant), IdentityStatus::VerifiedMatch);
    let uncovered = summary(&case, &job_id(&case, escaped.identity.pid));
    assert_eq!(report(&uncovered)["completeness"], "partial");
    assert_eq!(
        report(&uncovered)["missing_coverage"],
        json!(["unobserved_descendants"])
    );
    assert!(
        report(&uncovered)["cpu_core_seconds"]["total"]
            .as_f64()
            .unwrap()
            < 0.1
    );
    eventually(|| probe_live(&descendant) == IdentityStatus::Missing);
    save(
        "real_stop_continue_and_escaped_descendant_keep_explicit_partial_scope",
        json!({
        "stoppedUnknownSummary":during,"continuedSummary":stopped,"escapedSummary":uncovered,
        "escapedDescendantIdentity":descendant,"descendantOutlivedParent":true,"descendantFinallyAbsent":true,
        "singleExecutionCounts":[run.count(),escaped.count()]}),
    );
    case.stop("stop-escape-done");
}

#[test]
fn real_foreground_pipeline_excludes_unrelated_peer_cpu() {
    let _serial = serialization::serial();
    let mut case = setup("job-meter-foreground");
    let workload = case.fixture.root.join("native-workload");
    let compile = std::process::Command::new("clang")
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg(workspace().join("scripts/job-meter/native-workload.c"))
        .arg("-o")
        .arg(&workload)
        .output()
        .unwrap();
    assert!(compile.status.success(), "{compile:?}");
    let result = std::process::Command::new("python3")
        .arg(workspace().join("scripts/job-meter/foreground-reference.py"))
        .arg(case.fixture.root.join("foreground"))
        .arg(case.fixture.binary("arcctl"))
        .arg(&workload)
        .env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let observation: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let id = job_id(
        &case,
        observation["wrapper"]["pid"].as_u64().unwrap() as u32,
    );
    let surfaces = reconcile(&case, &id);
    let recorded = report(&surfaces["cli"]);
    assert_eq!(
        recorded["child"]["pid"],
        observation["reference"]["root"]["pid"]
    );
    let reference = observation["reference"]["total_core_seconds"]
        .as_f64()
        .unwrap();
    let actual = recorded["cpu_core_seconds"]["total"].as_f64().unwrap();
    let peer = observation["peerUserCoreSeconds"].as_f64().unwrap()
        + observation["peerSystemCoreSeconds"].as_f64().unwrap();
    assert!((actual - reference).abs() <= 0.1f64.max(reference * 0.05));
    assert!(peer > 0.1 && (actual - (reference + peer)).abs() > 0.1);
    assert_eq!(
        rows(&case, "SELECT count(*) AS count FROM job_lifecycle")[0]["count"],
        1
    );
    save(
        "real_foreground_pipeline_excludes_unrelated_peer_cpu",
        json!({"pipeline":observation,"surfaces":surfaces,
        "excludedPeerCoreSeconds":peer,"referenceCoreSeconds":reference,"recordedCoreSeconds":actual,
        "onlyOwnedChildAttributed":true}),
    );
    case.stop("foreground-done");
}

#[test]
fn real_job_summary_and_database_preserve_command_redaction() {
    let _serial = serialization::serial();
    let mut case = setup("job-meter-redaction");
    let sentinel = "job-meter-argv-environment-cwd-secret";
    let cwd = case.fixture.root.join(sentinel);
    fs::create_dir(&cwd).unwrap();
    let mut c = command(&case, false, "work");
    c.arg(sentinel)
        .env("PRIVATE_ENV", sentinel)
        .current_dir(&cwd);
    let mut run = Run::launch(&case, c);
    assert_eq!(run.finish().code(), Some(17));
    let surfaces = reconcile(&case, &job_id(&case, run.identity.pid));
    assert!(!surfaces.to_string().contains(sentinel));
    for path in [
        &case.fixture.database,
        &case.fixture.root.join("arc.sqlite3-wal"),
        &case.fixture.root.join("stdout"),
        &case.fixture.root.join("stderr"),
    ] {
        if let Ok(bytes) = fs::read(path) {
            assert!(!bytes
                .windows(sentinel.len())
                .any(|w| w == sentinel.as_bytes()));
        }
    }
    assert!(run.output().is_empty() && run.errors().is_empty());
    save(
        "real_job_summary_and_database_preserve_command_redaction",
        json!({"surfaces":surfaces,
        "singleExecutionCount":run.count(),"argvEnvironmentCwdSentinelAbsent":true,
        "readOnlyDatabaseWalAndDaemonLogsChecked":true}),
    );
    case.stop("redaction-done");
}
