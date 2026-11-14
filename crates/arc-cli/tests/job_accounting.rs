//! Real native children and the production arcctl binary, with deterministic
//! protocol peers. Durable daemon/SQLite/restart acceptance belongs to task-8.
#[path = "job_accounting/support.rs"]
mod job_accounting_support;
use arc_core::job_meter::{
    AccountingRelation, Completeness, MissingCoverageReason, TerminalOutcome,
};
use job_accounting_support::*;
#[allow(dead_code)]
#[path = "../../arc-daemon/tests/support/native_fixture.rs"]
mod native;
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
        // This is a fixture escape limit, not the product's report budget.
        // Independent queue exclusion and native CPU/outcome checks stay below.
        let elapsed = report.run_elapsed.seconds.unwrap();
        println!(
            "accounting fixture scheduled={scheduled} mode={mode} elapsed={elapsed:.6}s bound=60s"
        );
        assert!(elapsed < 60.0);
        if let Some(root) = std::env::var_os("ARC_TEST_EVIDENCE_DIR") {
            let root = std::path::Path::new(&root);
            fs::create_dir_all(root).unwrap();
            fs::write(root.join(format!("accounting-{}-{scheduled}-{mode}.json", std::process::id())),
                serde_json::to_vec(&serde_json::json!({"test":"lifecycle_and_scheduled_commands_report_exactly_one_native_exit", "scheduled":scheduled, "mode":mode, "elapsedSeconds":elapsed, "boundSeconds":60, "exitCode":code})).unwrap()).unwrap();
        }
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
    let owners = f.root.join("owners");
    command.env("ARC_FIXTURE_OWNERS", &owners);
    command
        .env("ARC_METER_TEST_READY", f.root.join("ready"))
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = native::OwnedChild::spawn_with_descendants(command, owners).unwrap();
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    child.child.id(),
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
    assert_eq!(
        unsafe { libc::kill(child.child.id() as i32, libc::SIGCONT) },
        0
    );
    assert_eq!(child.child.wait().unwrap().code(), Some(17));
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
        let mut command = f.command(false, "hold");
        command
            .env("ARC_METER_TEST_READY", &ready)
            .env("ARC_METER_TEST_RELEASE", &release)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child =
            native::OwnedChild::spawn_with_descendants(command, f.root.join("owners")).unwrap();
        let setup_deadline = Instant::now() + Duration::from_secs(30);
        while !ready.exists() || !f.state.lock().unwrap().attached {
            if Instant::now() >= setup_deadline {
                // Unwind through the descendant owner before fixture removal.
                panic!("{mode:?}: child readiness/attachment timed out");
            }
            thread::sleep(Duration::from_millis(1));
        }
        // The held child/attachment synchronizes setup. Exact absolute report
        // deadlines are asserted with an injected clock in exec::accounting::tests.
        fs::write(release, b"run").unwrap();
        let status = child.child.wait().unwrap();
        assert_eq!(status.code(), Some(17), "{mode:?}: {status:?}");
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
        let mut command = f.command_argv(&argv, "quick");
        command.stdout(Stdio::null()).stderr(Stdio::null());
        let mut child =
            native::OwnedChild::spawn_with_descendants(command, f.root.join("owners")).unwrap();
        while !f.root.join("marker").exists() {
            assert!(
                // Exact deadline identity across both fallback routes is
                // asserted without elapsed time in transport::preparation_tests.
                // This guard includes process startup and native filesystem I/O.
                start.elapsed() < Duration::from_secs(60),
                "native fail-open child did not start within the completion guard"
            );
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(child.child.wait().unwrap().code(), Some(17));
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

#[test]
fn stopped_wrapper_unwind_terminates_and_reaps_its_separately_grouped_target() {
    use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
    let f = Fixture::new(PeerMode::Normal);
    let (wrapper, target) = (std::cell::RefCell::new(None), std::cell::RefCell::new(None));
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let owners = f.root.join("unwind-owners");
        let mut command = f.command(false, "stop");
        command
            .env("ARC_FIXTURE_OWNERS", &owners)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = native::OwnedChild::spawn_with_descendants(command, owners.clone()).unwrap();
        *wrapper.borrow_mut() = Some(ProcessIdentity::capture(child.id()).unwrap());
        let end = Instant::now() + Duration::from_secs(60);
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
            assert!(Instant::now() < end, "wrapper stop handshake");
            thread::sleep(Duration::from_millis(1));
        }
        let path = fs::read_dir(owners)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        *target.borrow_mut() =
            Some(serde_json::from_slice::<ProcessIdentity>(&fs::read(path).unwrap()).unwrap());
        assert_eq!(
            probe_live(target.borrow().as_ref().unwrap()),
            IdentityStatus::VerifiedMatch
        );
        panic!("unwind at stopped-child phase before SIGCONT");
    }));
    assert!(unwind.is_err());
    for identity in [
        wrapper.borrow().clone().unwrap(),
        target.borrow().clone().unwrap(),
    ] {
        assert_eq!(
            probe_live(&identity),
            IdentityStatus::Missing,
            "{identity:?}"
        );
    }
    assert!(f.root.exists(), "cleanup precedes fixture removal");
}

#[test]
fn unwind_before_identity_publication_owns_the_separately_grouped_target() {
    use arc_core::process_identity::ProcessIdentity;
    let f = Fixture::new(PeerMode::Normal);
    let ready = f.root.join("before-identity");
    let owners = f.root.join("before-owners");
    let target = std::cell::RefCell::new(None);
    let wrapper = std::cell::Cell::new(0);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut command = f.command(false, "stop");
        command
            .env("ARC_FIXTURE_BEFORE_IDENTITY", &ready)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = native::OwnedChild::spawn_with_descendants(command, owners.clone()).unwrap();
        wrapper.set(child.id());
        let end = Instant::now() + Duration::from_secs(60);
        loop {
            if let Ok(pid) = fs::read_to_string(&ready)
                .unwrap_or_default()
                .parse::<u32>()
            {
                *target.borrow_mut() = Some(ProcessIdentity::capture(pid).unwrap());
                // Noninteractive exec uses a separate target process group.
                assert_ne!(unsafe { libc::getpgid(pid as i32) }, unsafe {
                    libc::getpgid(child.id() as i32)
                });
                break;
            }
            assert!(Instant::now() < end, "pre-publication checkpoint");
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(fs::read_dir(&owners).unwrap().count(), 0);
        assert!(!f.root.join("marker").exists());
        panic!("unwind before identity publication");
    }));
    assert!(unwind.is_err());
    assert!(ProcessIdentity::capture(target.borrow().as_ref().unwrap().pid).is_err());
    assert_eq!(
        unsafe { libc::waitpid(wrapper.get() as i32, std::ptr::null_mut(), libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
    assert!(f.root.exists());
}

#[test]
fn drop_after_wrapper_exit_waits_for_escaped_descendant_termination_and_reaping() {
    use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
    let f = Fixture::new(PeerMode::Normal);
    let escaped = f.root.join("escaped");
    let mut command = f.command(false, "escaped");
    command
        .env("ARC_METER_TEST_ESCAPED", &escaped)
        .env(
            "ARC_METER_TEST_ESCAPE_RELEASE",
            f.root.join("never-released"),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child =
        native::OwnedChild::spawn_with_descendants(command, f.root.join("owners")).unwrap();
    assert!(child.wait().unwrap().success());
    let target: ProcessIdentity = serde_json::from_slice(&fs::read(&escaped).unwrap()).unwrap();
    assert_eq!(probe_live(&target), IdentityStatus::VerifiedMatch);
    drop(child);
    assert!(
        ProcessIdentity::capture(target.pid).is_err(),
        "escaped target remains (possibly zombie)"
    );
    assert!(
        f.root.exists(),
        "descendant cleanup completes before fixture removal"
    );
}
