use super::{serialization, support::*};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    os::unix::{
        net::{UnixListener, UnixStream},
        process::ExitStatusExt,
    },
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn reference_cpu(case: &super::daemon_bridge_support::RealDaemon, mode: &str) -> f64 {
    let mut command = Command::new(case.fixture.binary("arc-exec-fixture"));
    command
        .env_clear()
        .env("ARC_FIXTURE_ACCOUNTING", mode)
        .env("HOME", &case.fixture.root)
        .env("TMPDIR", &case.fixture.root)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let mut child = super::native::OwnedChild::new(command.spawn().unwrap());
    let (status, usage) = super::native::native_readiness::wait4_usage(&mut child.child)
        .expect("same-run CPU reference child did not exit");
    // wait4 consumed the exit and usage; complete Child's ownership protocol.
    assert_eq!(
        child.child.try_wait().unwrap_err().raw_os_error(),
        Some(libc::ECHILD)
    );
    assert!(libc::WIFEXITED(status));
    assert_eq!(
        libc::WEXITSTATUS(status),
        if mode == "success" { 0 } else { 17 }
    );
    let seconds = |value: libc::timeval| value.tv_sec as f64 + value.tv_usec as f64 / 1e6;
    seconds(usage.ru_utime) + seconds(usage.ru_stime)
}

fn accept_control(listener: &UnixListener, run: &mut Run) -> UnixStream {
    listener.set_nonblocking(true).unwrap();
    let end = Instant::now() + Duration::from_secs(60);
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                break stream;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    run.wrapper.child.try_wait().unwrap().is_none(),
                    "child exited before control connection: {}",
                    run.errors()
                );
                assert!(
                    Instant::now() < end,
                    "child never connected its meter control socket"
                );
                thread::yield_now();
            }
            Err(e) => panic!("control accept: {e}"),
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let mut ready = [0u8; 5];
    stream
        .read_exact(&mut ready)
        .expect("child never sent meter control readiness");
    assert_eq!(&ready, b"ready");
    stream
}

fn observe_owned(run: &mut Run, mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(60);
    while !condition() {
        assert!(
            run.wrapper.child.try_wait().unwrap().is_none(),
            "child exited before observation: {}",
            run.errors()
        );
        assert!(
            Instant::now() < end,
            "meter child never reached the requested observation"
        );
        thread::yield_now();
    }
}

fn observe_descendant(
    identity: &ProcessIdentity,
    want_live: bool,
    mut condition: impl FnMut() -> bool,
) {
    let end = Instant::now() + Duration::from_secs(60);
    while !condition() {
        if want_live {
            assert_eq!(
                probe_live(identity),
                IdentityStatus::VerifiedMatch,
                "descendant exited before observation"
            );
        }
        assert!(
            Instant::now() < end,
            "meter descendant never reached the requested observation: {identity:?}"
        );
        thread::yield_now();
    }
}

#[test]
fn real_scheduled_lifecycle_exit_signal_and_partial_summaries() {
    let _serial = serialization::serial();
    let mut case = setup("job-meter-lifecycle");
    let mut observations = vec![];
    for scheduled in [false, true] {
        for (mode, code) in [("success", Some(0)), ("work", Some(17)), ("signal", None)] {
            let reference = code.map(|_| reference_cpu(&case, mode));
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
            if let Some(reference) = reference {
                let recorded = report["cpu_core_seconds"]["total"].as_f64().unwrap();
                assert!(reference > 0.0);
                assert!(
                    recorded >= reference / 4.0 && recorded <= reference * 4.0,
                    "recorded CPU {recorded}, same-run reference {reference}"
                );
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
    let mut case = setup_clocked("job-meter-queue");
    let first_path = case.fixture.root.join("first-control");
    let queued_path = case.fixture.root.join("queued-control");
    let first_listener = UnixListener::bind(&first_path).unwrap();
    let queued_listener = UnixListener::bind(&queued_path).unwrap();
    let mut first_command = command(&case, true, "channel-hold");
    first_command.env("ARC_METER_TEST_CONTROL", &first_path);
    let mut first = Run::launch(&case, first_command);
    let mut first_control = accept_control(&first_listener, &mut first);
    observe_owned(&mut first, || {
        rows(&case, "SELECT state FROM traffic_entries")
            .iter()
            .any(|r| r["state"] == "RUNNING")
    });
    let (mut queued_command, clock) = clocked_command(&case, true, "channel-hold");
    queued_command.env("ARC_METER_TEST_CONTROL", &queued_path);
    let mut queued = Run::launch(&case, queued_command);
    let queued_pid = queued.identity.pid;
    observe_owned(&mut queued, || {
        rows(&case, "SELECT t.state,l.observer_pid FROM traffic_entries t JOIN job_lifecycle l ON l.job_id=t.job_id")
        .iter().any(|r|r["observer_pid"]==queued_pid && r["state"]=="QUEUED")
    });
    assert!(!queued.marker.exists(), "queued work must not start");
    let id = job_id(&case, queued_pid);
    // Advance the fixture daemon's wall clock after observing the queued row.
    // The production admission code writes both timestamps to SQLite.
    let daemon_clock = case.fixture.root.join("daemon-clock");
    let queued_at: u64 = fs::read_to_string(&daemon_clock).unwrap().parse().unwrap();
    set_clock(&daemon_clock, queued_at + 5000);
    set_clock(&clock, 3600);
    first_control.write_all(b"r").unwrap();
    let mut queued_control = accept_control(&queued_listener, &mut queued);
    assert_eq!(queued.count(), 1);
    observe_owned(&mut queued, || {
        rows(&case, "SELECT t.state,l.observer_pid FROM traffic_entries t JOIN job_lifecycle l ON l.job_id=t.job_id")
        .iter().any(|r|r["observer_pid"]==queued_pid && r["state"]=="RUNNING")
    });
    set_clock(&clock, 3607);
    queued_control.write_all(b"r").unwrap();
    assert_eq!(first.finish().code(), Some(17));
    assert_eq!(queued.finish().code(), Some(17));
    let surfaces = reconcile(&case, &id);
    let summary = &surfaces["cli"];
    let queue = summary["queue_wait"]["seconds"].as_f64().unwrap();
    let run = report(summary)["run_elapsed"]["seconds"].as_f64().unwrap();
    // The fixtures compile production execution and admission paths with
    // injected clocks; reports still pass through SQLite and the bridge.
    assert_eq!(queue, 5.0, "controlled daemon queue interval");
    assert_eq!(
        run, 7.0,
        "queue time must not enter the native run interval"
    );
    assert_eq!(report(summary)["run_elapsed"]["clock"], "monotonic");
    assert_eq!(report(summary)["run_elapsed"]["includes_stopped"], true);
    assert_eq!(summary["queue_wait"]["basis"], "queued_to_reserved");
    assert_eq!(summary["queue_wait"]["clock"], "daemon_wall_clock");
    assert_eq!(queued.count(), 1);
    assert_eq!(first.count(), 1);
    let raw = rows(
        &case,
        "SELECT job_id,queued_at_ms,granted_at_ms FROM traffic_entries",
    );
    let row = raw.iter().find(|r| r["job_id"] == id).unwrap();
    assert_eq!(row["queued_at_ms"], queued_at);
    assert_eq!(row["granted_at_ms"], queued_at + 5000);
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
        "rawQueueTimestamps":raw,"queueSeconds":queue,"runSeconds":run,"clockFixture":true,
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
            let reference = reference_cpu(&case, "work");
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
            assert!(
                reference > 0.0
                    && inclusive_nested >= reference / 4.0
                    && inclusive_nested <= reference * 4.0,
                "nested CPU {inclusive_nested}, same-run work reference {reference}"
            );
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
    let control_path = case.fixture.root.join("stopped-control");
    let listener = UnixListener::bind(&control_path).unwrap();
    let (mut stopped_command, clock) = clocked_command(&case, false, "controlled-stop");
    stopped_command.env("ARC_METER_TEST_CONTROL", &control_path);
    let mut run = Run::launch(&case, stopped_command);
    let mut control = accept_control(&listener, &mut run);
    control.write_all(b"r").unwrap();
    let pid = run.identity.pid;
    observe_owned(&mut run, || {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    pid,
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
    set_clock(&clock, 7);
    signal_owned(&run.identity, libc::SIGCONT);
    assert_eq!(run.finish().code(), Some(17));
    let stopped = summary(&case, &id);
    assert_eq!(report(&stopped)["run_elapsed"]["seconds"], 7.0);
    assert_eq!(report(&stopped)["run_elapsed"]["includes_stopped"], true);
    let escaped_path = case.fixture.root.join("escaped.json");
    let mut c = command(&case, false, "escaped");
    let release = case.fixture.root.join("escape-release");
    let parent_cpu_path = case.fixture.root.join("escape-parent-cpu");
    let descendant_cpu_path = case.fixture.root.join("escape-descendant-cpu");
    c.env("ARC_METER_TEST_ESCAPED", &escaped_path)
        .env("ARC_METER_TEST_ESCAPE_RELEASE", &release)
        .env("ARC_METER_TEST_ESCAPE_PARENT_CPU", &parent_cpu_path)
        .env("ARC_METER_TEST_ESCAPE_DESCENDANT_CPU", &descendant_cpu_path);
    let mut escaped = Run::launch(&case, c);
    assert!(escaped.finish().success());
    let descendant: ProcessIdentity =
        serde_json::from_slice(&fs::read(&escaped_path).unwrap()).unwrap();
    escaped.wrapper.own_process(descendant.clone());
    assert_eq!(probe_live(&descendant), IdentityStatus::VerifiedMatch);
    let uncovered = summary(&case, &job_id(&case, escaped.identity.pid));
    assert_eq!(report(&uncovered)["completeness"], "partial");
    assert_eq!(
        report(&uncovered)["missing_coverage"],
        json!(["unobserved_descendants"])
    );
    observe_descendant(&descendant, true, || descendant_cpu_path.exists());
    let parent_cpu: f64 = fs::read_to_string(parent_cpu_path)
        .unwrap()
        .parse()
        .unwrap();
    let descendant_cpu: f64 = fs::read_to_string(descendant_cpu_path)
        .unwrap()
        .parse()
        .unwrap();
    let recorded = report(&uncovered)["cpu_core_seconds"]["total"]
        .as_f64()
        .unwrap();
    assert!(
        parent_cpu > 0.0 && recorded >= parent_cpu && recorded <= 4.0 * parent_cpu,
        "recorded {recorded}, same-run parent reference {parent_cpu}"
    );
    assert!(
        descendant_cpu > 4.0 * recorded,
        "escaped work {descendant_cpu} must be distinguishable from recorded {recorded}"
    );
    fs::write(release, b"release").unwrap();
    observe_descendant(&descendant, false, || {
        probe_live(&descendant) == IdentityStatus::Missing
    });
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
