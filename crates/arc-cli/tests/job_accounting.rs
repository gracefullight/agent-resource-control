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

#[path = "job_accounting/lineage.rs"]
mod lineage;

#[test]
fn stop_continue_time_is_included_and_native_signal_is_reraised_after_reporting() {
    use std::{io::Write, os::unix::net::UnixListener};
    let f = Fixture::new(PeerMode::Normal);
    let control_path = f.root.join("stop-control");
    let listener = UnixListener::bind(&control_path).unwrap();
    let mut command = f.clocked_command(false, "controlled-stop");
    let owners = f.root.join("owners");
    command.env("ARC_FIXTURE_OWNERS", &owners);
    command
        .env("ARC_METER_TEST_CONTROL", &control_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = native::OwnedChild::spawn_with_descendants(command, owners).unwrap();
    let mut control = lineage::attached_control(&f, &listener);
    control.write_all(b"r").unwrap();
    loop {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.child.id(),
                &mut info,
                libc::WSTOPPED | libc::WEXITED | libc::WNOWAIT,
            )
        };
        if result < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        assert_eq!(result, 0);
        assert_eq!(
            info.si_code,
            libc::CLD_STOPPED,
            "wrapper exited without suspending"
        );
        break;
    }
    let pending_clock = f.root.join("clock.pending");
    fs::write(&pending_clock, b"7").unwrap();
    fs::rename(pending_clock, f.clock_path()).unwrap();
    assert_eq!(
        unsafe { libc::kill(child.child.id() as i32, libc::SIGCONT) },
        0
    );
    assert_eq!(child.child.wait().unwrap().code(), Some(17));
    let state = f.state.lock().unwrap();
    let report = state.reports.values().next().unwrap();
    assert!(report.run_elapsed.includes_stopped);
    assert_eq!(report.run_elapsed.seconds, Some(7.0));
    assert!(report.cpu_core_seconds.total.unwrap() < report.run_elapsed.seconds.unwrap());
    drop(state);
    let signal_fixture = Fixture::new(PeerMode::Normal);
    let output = lineage::synchronized_output(
        &signal_fixture,
        signal_fixture.clocked_command(false, "controlled-signal"),
        1,
    );
    assert_eq!(output.status.signal(), Some(libc::SIGTERM));
    assert!(signal_fixture.state.lock().unwrap().reports.values().any(|r| matches!(r.outcome, TerminalOutcome::Signaled {signal, ..} if signal == libc::SIGTERM as u8)));
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
fn fallback_after_bypass_preserves_single_execution_when_registration_is_unavailable() {
    for (mode, argv) in [
        (PeerMode::BypassThenUnavailable, ["cargo", "build"]),
        (PeerMode::UnsupportedBunThenUnavailable, ["bun", "test"]),
    ] {
        let f = Fixture::new(mode);
        let mut command = f.command_argv(&argv, "quick");
        command.stdout(Stdio::null()).stderr(Stdio::null());
        let mut child =
            native::OwnedChild::spawn_with_descendants(command, f.root.join("owners")).unwrap();
        // The exact pre-spawn deadline, including a delayed bypass, is driven
        // by a controlled clock in transport::preparation_tests.
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
    use std::{
        io::{BufRead, Read, Write},
        os::unix::net::UnixListener,
        sync::mpsc,
    };
    let f = Fixture::new(PeerMode::Queued);
    let control_path = f.root.join("c");
    let listener = UnixListener::bind(&control_path).unwrap();
    let mut command = f.clocked_command(true, "channel-hold");
    command
        .env("ARC_FIXTURE_CONTROLLED_NOTICE", "1")
        .env("ARC_METER_TEST_CONTROL", &control_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child =
        native::OwnedChild::spawn_with_descendants(command, f.root.join("owners")).unwrap();
    let stderr = child.child.stderr.take().unwrap();
    let (seen, observed) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut lines = std::io::BufReader::new(stderr);
        let mut diagnostics = String::new();
        let (mut queued_seen, mut started_seen) = (false, false);
        loop {
            let mut line = String::new();
            if lines.read_line(&mut line).unwrap() == 0 {
                break;
            }
            diagnostics.push_str(&line);
            if !queued_seen && diagnostics.contains("QUEUED\nWaiting to start.") {
                seen.send("queued").unwrap();
                queued_seen = true;
            }
            if !started_seen && diagnostics.contains("STARTED\nCommand started.") {
                seen.send("started").unwrap();
                started_seen = true;
            }
        }
        diagnostics
    });
    f.wait_for_queued_poll();
    assert!(
        !f.root.join("marker").exists(),
        "queued work must not start before release"
    );
    assert!(child.child.try_wait().unwrap().is_none());
    assert_eq!(
        observed
            .recv_timeout(Duration::from_secs(60))
            .expect("queued notice was not observed"),
        "queued"
    );
    assert!(!f.root.join("marker").exists());
    let pending = f.root.join("clock.pending");
    fs::write(&pending, b"3600").unwrap();
    fs::rename(&pending, f.clock_path()).unwrap();
    f.release_queued();
    assert_eq!(
        observed
            .recv_timeout(Duration::from_secs(60))
            .expect("started notice was not observed"),
        "started"
    );
    listener.set_nonblocking(true).unwrap();
    let end = Instant::now() + Duration::from_secs(60);
    let mut control = loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                break stream;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    child.child.try_wait().unwrap().is_none(),
                    "child exited before control connection"
                );
                assert!(
                    Instant::now() < end,
                    "child never connected its accounting control socket"
                );
                thread::yield_now();
            }
            Err(error) => panic!("child control accept: {error}"),
        }
    };
    control
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let mut ready = [0u8; 5];
    control
        .read_exact(&mut ready)
        .expect("child never sent accounting control readiness");
    assert_eq!(&ready, b"ready");
    f.wait_for_attachment();
    assert!(child.child.try_wait().unwrap().is_none());
    fs::write(&pending, b"3607").unwrap();
    fs::rename(&pending, f.clock_path()).unwrap();
    control.write_all(b"r").unwrap();
    assert_eq!(child.wait_with_output().unwrap().status.code(), Some(17));
    let diagnostics = reader.join().unwrap();
    let state = f.state.lock().unwrap();
    let report = state.reports.values().next().unwrap();
    assert_eq!(report.run_elapsed.seconds, Some(7.0));
    assert_eq!(report.outcome, TerminalOutcome::Exited { code: 17 });
    assert_eq!(
        fs::read_to_string(f.root.join("marker"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert!(
        diagnostics.contains("QUEUED\nWaiting to start."),
        "queued notice missing: {diagnostics:?}"
    );
    assert!(
        diagnostics.contains("STARTED\nCommand started."),
        "started notice missing: {diagnostics:?}"
    );
}

#[test]
fn a_nested_job_after_parent_registration_outage_stays_unverified() {
    let f = Fixture::new(PeerMode::UntrackedRoot);
    let reference = lineage::direct_work_reference(&f, "work", 17);
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
    let nested_cpu = nested.cpu_core_seconds.total.unwrap();
    assert!(
        reference > 0.0 && nested_cpu >= reference / 4.0 && nested_cpu <= reference * 4.0,
        "nested CPU {nested_cpu}, same-run work reference {reference}"
    );
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
