//! Real native children and the production arcctl binary, with deterministic
//! protocol peers. Durable daemon/SQLite/restart acceptance belongs to task-8.
#[path = "job_accounting/support.rs"]
mod job_accounting_support;
use arc_core::job_meter::{
    AccountingRelation, Completeness, MissingCoverageReason, TerminalOutcome,
};
use job_accounting_support::*;
use std::{
    fs,
    os::unix::process::ExitStatusExt,
    process::Stdio,
    thread,
    time::{Duration, Instant},
};

#[test]
fn lifecycle_and_scheduled_commands_report_exactly_one_native_exit() {
    for (scheduled, mode, code) in [
        (false, "work", 17),
        (true, "work", 17),
        (false, "success", 0),
        (true, "success", 0),
    ] {
        let f = Fixture::new(PeerMode::Normal);
        let output = f.command(scheduled, mode).output().unwrap();
        assert_eq!(output.status.code(), Some(code), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty(), "{output:?}");
        let state = f.state.lock().unwrap();
        assert_eq!(state.reports.len(), 1);
        let report = state.reports.values().next().unwrap();
        assert_eq!(report.outcome, TerminalOutcome::Exited { code: code as u8 });
        assert!(report.cpu_core_seconds.user.unwrap() > 0.01);
        assert!(report.cpu_core_seconds.system.unwrap() >= 0.0);
        assert!(
            (report.cpu_core_seconds.total.unwrap()
                - (report.cpu_core_seconds.user.unwrap()
                    + report.cpu_core_seconds.system.unwrap()))
            .abs()
                <= 1e-9
        );
        assert!(report.run_elapsed.seconds.unwrap() >= 0.12);
        assert!(report.run_elapsed.seconds.unwrap() < 1.0);
        assert_eq!(report.completeness, Completeness::Partial);
        assert!(report
            .missing_coverage
            .contains(&MissingCoverageReason::UnobservedDescendants));
        assert!(
            matches!(&report.accounting_relation, AccountingRelation::CanonicalRoot {root_job_id} if root_job_id == &report.job_id)
        );
        assert_eq!(
            fs::read_to_string(f.root.join("marker"))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }
}
#[test]
fn nested_wrappers_resolve_one_canonical_root_across_scheduled_and_lifecycle_modes() {
    for (outer_scheduled, nested_mode) in [
        (false, "nested"),
        (true, "nested"),
        (false, "nested-scheduled"),
        (true, "nested-scheduled"),
    ] {
        let f = Fixture::new(PeerMode::Normal);
        let output = f.command(outer_scheduled, nested_mode).output().unwrap();
        assert_eq!(output.status.code(), Some(17), "{output:?}");
        let state = f.state.lock().unwrap();
        assert_eq!(state.reports.len(), 2, "{state:?}");
        let root = state
            .reports
            .values()
            .find(|r| r.accounting_relation.is_additive())
            .unwrap();
        let nested = state
            .reports
            .values()
            .find(|r| !r.accounting_relation.is_additive())
            .unwrap();
        assert_eq!(
            nested.accounting_relation,
            AccountingRelation::IncludedInRoot {
                root_job_id: root.job_id.clone()
            }
        );
        assert!(
            root.cpu_core_seconds.total.unwrap() + 0.001 >= nested.cpu_core_seconds.total.unwrap()
        );
        assert!(
            root.cpu_core_seconds.total.unwrap()
                < nested.cpu_core_seconds.total.unwrap() * 1.5 + 0.02
        );
        assert_eq!(
            fs::read_to_string(f.root.join("marker"))
                .unwrap()
                .lines()
                .count(),
            2
        );
    }
}
#[test]
fn unverified_lineage_is_non_additive_and_caller_environment_does_not_create_ownership() {
    let f = Fixture::new(PeerMode::Normal);
    let output = f
        .command(false, "quick")
        .env("ARC_TRAFFIC_LINEAGE", "f".repeat(64))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(17));
    let state = f.state.lock().unwrap();
    let report = state.reports.values().next().unwrap();
    assert!(matches!(
        report.accounting_relation,
        AccountingRelation::CanonicalRoot { .. }
    ));
    drop(state);
    let observer =
        arc_core::process_identity::ProcessIdentity::capture(std::process::id()).unwrap();
    let _unknown = arc_platform::job_lineage::Record::create(
        &f.socket,
        &observer,
        &"e".repeat(32),
        &AccountingRelation::Unverified {},
    )
    .unwrap();
    let output = f.command(false, "work").output().unwrap();
    assert_eq!(output.status.code(), Some(17));
    let state = f.state.lock().unwrap();
    let uncertain = state
        .reports
        .values()
        .find(|r| matches!(r.accounting_relation, AccountingRelation::Unverified {}))
        .unwrap();
    assert!(!uncertain.accounting_relation.is_additive());
    assert!(uncertain
        .missing_coverage
        .contains(&MissingCoverageReason::LineageUnverified));
}
#[test]
fn stop_continue_time_is_included_and_native_signal_is_reraised_after_reporting() {
    let f = Fixture::new(PeerMode::Normal);
    let mut command = f.command(false, "stop");
    command
        .env("ARC_METER_TEST_READY", f.root.join("ready"))
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command.spawn().unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    loop {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    child.id(),
                    &mut info,
                    libc::WSTOPPED | libc::WNOHANG | libc::WNOWAIT,
                )
            },
            0
        );
        if info.si_code == libc::CLD_STOPPED {
            break;
        }
        assert!(Instant::now() < end, "wrapper did not suspend");
        thread::sleep(Duration::from_millis(1));
    }
    thread::sleep(Duration::from_millis(180));
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGCONT) }, 0);
    assert_eq!(child.wait().unwrap().code(), Some(17));
    let state = f.state.lock().unwrap();
    let report = state.reports.values().next().unwrap();
    assert!(report.run_elapsed.includes_stopped);
    assert!(report.run_elapsed.seconds.unwrap() >= 0.3);
    assert!(report.cpu_core_seconds.total.unwrap() < report.run_elapsed.seconds.unwrap());
    drop(state);
    let output = f.command(false, "signal").output().unwrap();
    assert_eq!(output.status.signal(), Some(libc::SIGTERM));
    assert!(f.state.lock().unwrap().reports.values().any(|r| matches!(r.outcome, TerminalOutcome::Signaled {signal, ..} if signal == libc::SIGTERM as u8)));
}
#[test]
fn reporting_outages_unsupported_peers_and_lost_acks_preserve_single_execution() {
    for mode in [
        PeerMode::LostReportAck,
        PeerMode::UnsupportedReport,
        PeerMode::StalledReport,
        PeerMode::SlowCompletion,
        PeerMode::MalformedReportAck,
    ] {
        let f = Fixture::new(mode);
        let release = f.root.join("release");
        let ready = f.root.join("ready");
        let mut child = f
            .command(false, "hold")
            .env("ARC_METER_TEST_READY", &ready)
            .env("ARC_METER_TEST_RELEASE", &release)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let setup_deadline = Instant::now() + Duration::from_secs(30);
        while !ready.exists() || !f.state.lock().unwrap().attached {
            if Instant::now() >= setup_deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("{mode:?}: child readiness/attachment timed out");
            }
            thread::sleep(Duration::from_millis(1));
        }
        // Measure the existing work + reporting bound after one-time process
        // setup. The held child and native attachment make this ordering explicit.
        let start = Instant::now();
        fs::write(release, b"run").unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(17), "{mode:?}: {output:?}");
        assert!(
            start.elapsed() < Duration::from_millis(1500),
            "{mode:?}: elapsed {:?}; {output:?}",
            start.elapsed()
        );
        assert_eq!(
            fs::read_to_string(f.root.join("marker"))
                .unwrap()
                .lines()
                .count(),
            1
        );
        let state = f.state.lock().unwrap();
        if matches!(mode, PeerMode::SlowCompletion) {
            assert!(state.report_requests.is_empty());
        }
        if matches!(mode, PeerMode::LostReportAck | PeerMode::MalformedReportAck) {
            assert_eq!(state.reports.len(), 1);
            assert!(state.summary_reads > 0);
            assert!(state
                .report_requests
                .windows(2)
                .all(|pair| pair[0] == pair[1]));
        }
    }
}
#[test]
fn fallback_after_slow_bypass_uses_the_original_pre_spawn_deadline() {
    for (mode, argv) in [
        (PeerMode::SlowBypassThenStalledRegister, ["cargo", "build"]),
        (
            PeerMode::SlowUnsupportedBunThenStalledRegister,
            ["bun", "test"],
        ),
    ] {
        let f = Fixture::new(mode);
        let start = Instant::now();
        let mut child = f
            .command_argv(&argv, "quick")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        while !f.root.join("marker").exists() {
            assert!(
                start.elapsed() < Duration::from_millis(2000),
                "pre-spawn deadline was renewed"
            );
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(child.wait().unwrap().code(), Some(17));
        assert!(
            f.state.lock().unwrap().report_requests.is_empty(),
            "unconfirmed registration cannot authorize an accounting report"
        );
        assert_eq!(
            fs::read_to_string(f.root.join("marker"))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }
}

#[test]
fn queued_wait_does_not_inflate_the_native_run_interval() {
    let f = Fixture::new(PeerMode::Queued);
    let start = Instant::now();
    let output = f.command(true, "work").output().unwrap();
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    let elapsed = start.elapsed().as_secs_f64();
    let state = f.state.lock().unwrap();
    let report = state.reports.values().next().unwrap();
    assert!(report.run_elapsed.seconds.unwrap() >= 0.12);
    assert!(report.run_elapsed.seconds.unwrap() < elapsed - 0.2);
    let diagnostics = String::from_utf8(output.stderr).unwrap();
    assert!(diagnostics.contains("QUEUED\nWaiting to start."));
    assert!(diagnostics.contains("STARTED\nCommand started."));
}

#[test]
fn a_nested_job_after_parent_registration_outage_stays_unverified() {
    let f = Fixture::new(PeerMode::UntrackedRoot);
    let output = f.command(false, "nested").output().unwrap();
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("TRACKING_UNCERTAIN"));
    let state = f.state.lock().unwrap();
    assert_eq!(
        state.reports.len(),
        1,
        "only the recovered nested job is attached"
    );
    let nested = state.reports.values().next().unwrap();
    assert_eq!(
        nested.accounting_relation,
        AccountingRelation::Unverified {}
    );
    assert!(!nested.accounting_relation.is_additive());
    assert!(nested.cpu_core_seconds.total.unwrap() > 0.01);
    assert!(nested
        .missing_coverage
        .contains(&MissingCoverageReason::LineageUnverified));
    assert_eq!(
        fs::read_to_string(f.root.join("marker"))
            .unwrap()
            .lines()
            .count(),
        2
    );
}
