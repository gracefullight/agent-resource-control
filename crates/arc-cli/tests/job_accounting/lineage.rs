use super::*;

pub(super) fn attached_control(
    f: &Fixture,
    listener: &std::os::unix::net::UnixListener,
) -> std::os::unix::net::UnixStream {
    use std::{io::Read, os::fd::AsRawFd};
    let mut poll = libc::pollfd {
        fd: listener.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    loop {
        let result = unsafe { libc::poll(&mut poll, 1, 60_000) };
        if result < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        assert!(result > 0, "native fixture connection watchdog");
        break;
    }
    let (mut control, _) = listener.accept().unwrap();
    control
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let mut ready = [0u8; 5];
    control.read_exact(&mut ready).unwrap();
    assert_eq!(&ready, b"ready");
    f.wait_for_attachment();
    control
}

pub(super) fn synchronized_output(
    f: &Fixture,
    command: std::process::Command,
    children: usize,
) -> std::process::Output {
    synchronized_output_impl(f, command, children, None)
}

fn synchronized_production_output(
    f: &Fixture,
    command: std::process::Command,
    children: usize,
    wrapped: bool,
) -> std::process::Output {
    synchronized_output_impl(f, command, children, Some(wrapped))
}

fn synchronized_output_impl(
    f: &Fixture,
    mut command: std::process::Command,
    children: usize,
    wrapped: Option<bool>,
) -> std::process::Output {
    use std::{
        io::{Read, Write},
        os::unix::net::UnixListener,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let socket = f
        .root
        .join(format!("c{}", NEXT.fetch_add(1, Ordering::Relaxed)));
    let listener = UnixListener::bind(&socket).unwrap();
    command
        .env("ARC_METER_TEST_CONTROL", &socket)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child =
        native::OwnedChild::spawn_with_descendants(command, f.root.join("controlled-owners"))
            .unwrap();
    for index in 0..children {
        let mut control = attached_control(f, &listener);
        if index == 0 {
            if let Some(wrapped) = wrapped {
                f.assert_live_parent(child.child.id(), wrapped);
            }
        }
        control.write_all(b"r").unwrap();
    }
    let status = child.child.wait().unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();
    child
        .child
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut stderr)
        .unwrap();
    fs::remove_file(socket).unwrap();
    std::process::Output {
        status,
        stdout,
        stderr,
    }
}

pub(super) fn direct_work_reference(f: &Fixture, mode: &str, code: i32) -> f64 {
    use arc_platform::child_wait::{wait_owned_child, WaitMode};
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_arc-exec-fixture"));
    command
        .env_clear()
        .env("ARC_FIXTURE_ACCOUNTING", mode)
        .env("ARC_FIXTURE_OWNERS", f.root.join("reference-owners"))
        .env("ARC_METER_TEST_MARKER", f.root.join("reference-marker"))
        .env("HOME", &f.root)
        .env("TMPDIR", &f.root)
        .env("PATH", &f.root)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let mut child = native::OwnedChild::new(command.spawn().unwrap());
    let event = loop {
        match wait_owned_child(child.child.id(), WaitMode::Blocking) {
            Ok(Some(event)) => break event,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => (),
            result => panic!("direct native reference: {result:?}"),
        }
    };
    assert_eq!(
        child.child.wait().unwrap_err().raw_os_error(),
        Some(libc::ECHILD)
    );
    assert_eq!(event.status.code(), Some(code));
    event.terminal_usage.unwrap().unwrap().user_core_seconds
}

#[test]
fn lifecycle_and_scheduled_commands_report_exactly_one_native_exit() {
    for (scheduled, mode, code) in [
        (false, "work", 17),
        (true, "work", 17),
        (false, "success", 0),
        (true, "success", 0),
    ] {
        for wrapped in [false, true] {
            let f = Fixture::new(PeerMode::Normal);
            let reference_cpu = direct_work_reference(&f, mode, code);
            let controlled_mode = format!("controlled-{mode}");
            let mut command = f.production_command(scheduled, &controlled_mode, wrapped);
            command
                .env("_", "/unrelated/caller")
                .env("ARC_TRAFFIC_LINEAGE", "f".repeat(64));
            let output = synchronized_production_output(&f, command, 1, wrapped);
            assert_eq!(output.status.code(), Some(code), "{output:?}");
            assert!(output.stdout.is_empty());
            assert!(output.stderr.is_empty(), "{output:?}");
            let state = f.state.lock().unwrap();
            assert_eq!(state.reports.len(), 1);
            let report = state.reports.values().next().unwrap();
            assert_eq!(report.outcome, TerminalOutcome::Exited { code: code as u8 });
            let user_cpu = report.cpu_core_seconds.user.unwrap();
            assert!(reference_cpu > 0.0);
            // Compare the same fixed workload on this host in this run. A 4x
            // tolerance accommodates CPU frequency and coverage instrumentation.
            assert!(
                user_cpu >= reference_cpu / 4.0 && user_cpu <= reference_cpu * 4.0,
                "native user CPU {user_cpu}, reference {reference_cpu}, tolerance 4x"
            );
            assert!(report.cpu_core_seconds.system.unwrap() >= 0.0);
            assert!(
                (report.cpu_core_seconds.total.unwrap()
                    - (report.cpu_core_seconds.user.unwrap()
                        + report.cpu_core_seconds.system.unwrap()))
                .abs()
                    <= 1e-9
            );
            assert!(report.run_elapsed.seconds.is_some_and(f64::is_finite));
            assert_eq!(report.completeness, Completeness::Partial);
            assert!(report
                .missing_coverage
                .contains(&MissingCoverageReason::UnobservedDescendants));
            assert!(
                matches!(&report.accounting_relation, AccountingRelation::CanonicalRoot {root_job_id} if root_job_id == &report.job_id),
                "root relation: {:?}; ancestry: {:?}",
                report.accounting_relation,
                state.ancestry_diagnostics
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
}
#[test]
fn nested_wrappers_resolve_one_canonical_root_across_scheduled_and_lifecycle_modes() {
    for (outer_scheduled, nested_mode) in [
        (false, "nested"),
        (true, "nested"),
        (false, "nested-scheduled"),
        (true, "nested-scheduled"),
    ] {
        for wrapped in [false, true] {
            let f = Fixture::new(PeerMode::Normal);
            let controlled_mode = format!("controlled-{nested_mode}");
            let mut command = f.production_command(outer_scheduled, &controlled_mode, wrapped);
            command
                .env("_", "/unrelated/caller")
                .env("ARC_TRAFFIC_LINEAGE", "f".repeat(64));
            let output = synchronized_production_output(&f, command, 2, wrapped);
            assert_eq!(output.status.code(), Some(17), "{output:?}");
            let state = f.state.lock().unwrap();
            assert_eq!(state.reports.len(), 2, "{state:?}");
            let root = state
                .reports
                .values()
                .find(|r| r.accounting_relation.is_additive())
                .unwrap_or_else(|| {
                    panic!(
                        "no canonical root; ancestry: {:?}",
                        state.ancestry_diagnostics
                    )
                });
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
                root.cpu_core_seconds.total.unwrap() + 0.001
                    >= nested.cpu_core_seconds.total.unwrap()
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
}
#[test]
fn unverified_lineage_is_non_additive_and_caller_environment_does_not_create_ownership() {
    for scheduled in [false, true] {
        for claim in ["", "unverified", &"f".repeat(64)] {
            for wrapped in [false, true] {
                let f = Fixture::new(PeerMode::Normal);
                let captured = f.root.join("inherited-environment.json");
                let mut command =
                    f.production_command(scheduled, "capture-controlled-quick", wrapped);
                command
                    .env("_", "/unrelated/caller")
                    .env("ARC_TRAFFIC_LINEAGE", claim)
                    .env("ARC_FIXTURE_CAPTURE_ENV", &captured);
                let output = synchronized_production_output(&f, command, 1, wrapped);
                assert_eq!(output.status.code(), Some(17));
                let environment: serde_json::Value =
                    serde_json::from_slice(&fs::read(captured).unwrap()).unwrap();
                assert_eq!(environment["lineage"], claim);
                assert_eq!(environment["underscore"], "/unrelated/caller");
                let state = f.state.lock().unwrap();
                assert_eq!(
                    state.reports.len(),
                    1,
                    "scheduled={scheduled}, claim={claim:?}, wrapped={wrapped}, stderr={:?}, state={state:?}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let report = state.reports.values().next().unwrap();
                assert!(
                    matches!(
                        &report.accounting_relation,
                        AccountingRelation::CanonicalRoot { root_job_id } if root_job_id == &report.job_id
                    ),
                    "scheduled={scheduled}, claim={claim:?}, relation={:?}, ancestry={:?}",
                    report.accounting_relation,
                    state.ancestry_diagnostics
                );
            }
        }
    }
    let f = Fixture::new(PeerMode::Normal);
    let observer =
        arc_core::process_identity::ProcessIdentity::capture(std::process::id()).unwrap();
    let _unknown = arc_platform::job_lineage::Record::create(
        &f.socket,
        &observer,
        &"e".repeat(32),
        &AccountingRelation::Unverified {},
    )
    .unwrap();
    let captured = f.root.join("unknown-ancestor-environment.json");
    let mut command = f.production_command(false, "capture-controlled-work", true);
    command
        .env("ARC_TRAFFIC_LINEAGE", "f".repeat(64))
        .env("_", "/unrelated/caller")
        .env("ARC_FIXTURE_CAPTURE_ENV", &captured);
    let output = synchronized_production_output(&f, command, 1, true);
    assert_eq!(output.status.code(), Some(17));
    let environment: serde_json::Value =
        serde_json::from_slice(&fs::read(captured).unwrap()).unwrap();
    assert_eq!(environment["lineage"], "f".repeat(64));
    assert_eq!(environment["underscore"], "/unrelated/caller");
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
fn accounting_mode_ignores_ambient_hook_and_capture_controls() {
    let f = Fixture::new(PeerMode::Normal);
    let capture = f.root.join("unrequested-capture");
    let hook_marker = f.root.join("unrequested-hook");
    let output = f
        .command(false, "quick")
        .env("ARC_FIXTURE_MODE", "hook-capture")
        .env("ARC_HOOK_CAPTURE", "1")
        .env("ARC_HOOK_CAPTURE_MARKER", &hook_marker)
        .env("ARC_FIXTURE_CAPTURE_ENV", &capture)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(!capture.exists());
    assert!(!hook_marker.exists());
    assert_eq!(
        fs::read_to_string(f.root.join("marker"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[test]
fn a_live_unverified_ancestor_changes_root_classification_without_environment_help() {
    let f = Fixture::new(PeerMode::Normal);
    let run = |wrapped: bool| {
        let mut command = f.production_command(false, "controlled-quick", wrapped);
        command
            .env("_", "/unrelated/caller")
            .env("ARC_TRAFFIC_LINEAGE", "f".repeat(64));
        assert_eq!(
            synchronized_production_output(&f, command, 1, wrapped)
                .status
                .code(),
            Some(17)
        );
    };
    run(false);
    let observer =
        arc_core::process_identity::ProcessIdentity::capture(std::process::id()).unwrap();
    let record = arc_platform::job_lineage::Record::create(
        &f.socket,
        &observer,
        &"e".repeat(32),
        &AccountingRelation::Unverified {},
    )
    .unwrap();
    run(true);
    drop(record);
    run(true);
    let state = f.state.lock().unwrap();
    assert_eq!(state.reports.len(), 3);
    assert_eq!(
        state
            .reports
            .values()
            .filter(|report| report.accounting_relation.is_additive())
            .count(),
        2
    );
    let unknown: Vec<_> = state
        .reports
        .values()
        .filter(|report| {
            matches!(
                report.accounting_relation,
                AccountingRelation::Unverified {}
            )
        })
        .collect();
    assert_eq!(unknown.len(), 1);
    assert!(unknown[0]
        .missing_coverage
        .contains(&MissingCoverageReason::LineageUnverified));
}

#[test]
fn controlled_fixture_waits_for_socket_release() {
    use std::{
        io::{Read, Write},
        os::{fd::AsRawFd, unix::net::UnixListener},
    };
    let f = Fixture::new(PeerMode::Normal);
    let socket = f.root.join("control");
    let listener = UnixListener::bind(&socket).unwrap();
    let mut command = f.command(true, "channel-hold");
    command
        .env("ARC_METER_TEST_CONTROL", &socket)
        .env("ARC_FIXTURE_MODE", "hook-capture")
        .env(
            "ARC_FIXTURE_CAPTURE_ENV",
            f.root.join("unrequested-capture"),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child =
        native::OwnedChild::spawn_with_descendants(command, f.root.join("owners")).unwrap();
    let mut poll = libc::pollfd {
        fd: listener.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    loop {
        let result = unsafe { libc::poll(&mut poll, 1, 60_000) };
        if result < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        assert!(result > 0, "fixture control connection watchdog");
        break;
    }
    let (mut control, _) = listener.accept().unwrap();
    control
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let mut ready = [0u8; 5];
    control.read_exact(&mut ready).unwrap();
    assert_eq!(&ready, b"ready");
    assert!(child.child.try_wait().unwrap().is_none());
    assert!(!f.root.join("unrequested-capture").exists());
    control.write_all(b"r").unwrap();
    assert_eq!(child.child.wait().unwrap().code(), Some(17));
    let state = f.state.lock().unwrap();
    assert_eq!(state.reports.len(), 1);
    assert_eq!(
        state.reports.values().next().unwrap().outcome,
        TerminalOutcome::Exited { code: 17 }
    );
}
