use super::*;
use arc_detector::{JobClass, Locality, Uncertainty};
use std::{fs, os::unix::fs::PermissionsExt, process::Command, time::Instant};

#[allow(clippy::duplicate_mod)]
#[path = "../../../../arc-daemon/tests/support/child_watchdog.rs"]
mod child_watchdog;

#[test]
fn oversized_and_unrelated_commands_keep_the_detector_budget_reason() {
    for program in ["custom-shell", "git"] {
        let argv = [
            OsString::from(program),
            OsString::from("x".repeat(arc_detector::MAX_INPUT_BYTES + 1)),
        ];
        let mut context = EndpointContext {
            captured_at_ms: 10_000,
            ..Default::default()
        };
        assert!(extend_with(&argv, &mut context, Duration::ZERO, |_| panic!(
            "unexpected capture"
        ))
        .is_some());
        let actual = arc_detector::classify_argv_with_context(&argv, &context, 10_000);
        assert_eq!(actual.reason, arc_detector::Reason::InputLimit);
    }
}

#[test]
fn stalled_capture_falls_back_and_executes_once() {
    const CHILD: &str = "ARC_TEST_CI_STALLED_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let classify = |argv: &[OsString]| {
            let now = arc_ipc::locality::now_ms().unwrap();
            let mut context = EndpointContext {
                captured_at_ms: now,
                ..Default::default()
            };
            let (entered, seen) = mpsc::channel();
            let (release, blocked) = mpsc::channel::<()>();
            let result = extend_with(argv, &mut context, Duration::from_millis(10), move |_| {
                entered.send(()).unwrap();
                blocked.recv().unwrap();
                Some(EndpointFact {
                    kind: EndpointKind::CiHost,
                    selector: "gh".into(),
                    endpoints: vec!["https://github.com".into()],
                })
            });
            assert!(result.is_none());
            seen.recv_timeout(Duration::from_secs(60))
                .expect("capture worker never reached stalled read");
            assert!(context.facts.is_empty());
            let actual = arc_detector::classify_argv_with_context(argv, &context, now);
            assert_eq!(
                (actual.classification, actual.locality, actual.uncertainty),
                (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe)
            );
            release.send(()).unwrap();
            actual
        };
        let code = super::super::run::run_with_classifier(
            ["gh", "workflow", "run", "build.yml"]
                .map(OsString::from)
                .to_vec(),
            false,
            &Instant::now,
            &Instant::now,
            &classify,
        );
        assert_eq!(code, std::process::ExitCode::from(23));
        return;
    }

    let root = std::env::temp_dir().join(format!("arc-ci-stall-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let stub = root.join("gh");
    fs::write(
        &stub,
        "#!/bin/sh\nprintf 'call\\n' >> \"$MARKER\"\nexit 23\n",
    )
    .unwrap();
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o700)).unwrap();
    let marker = root.join("calls");
    let mut child = Command::new(std::env::current_exe().unwrap());
    child
        .args([
            "--exact",
            "exec::ci_context::tests::stalled_capture_falls_back_and_executes_once",
            "--nocapture",
        ])
        .env_clear()
        .env(CHILD, "1")
        .env("HOME", &root)
        .env("PATH", &root)
        .env("MARKER", &marker)
        .env(arc_core::ARC_SOCKET_PATH, root.join("missing.sock"));
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        child.env("LLVM_PROFILE_FILE", profile);
    }
    let output = child_watchdog::output(&mut child, "stalled CI capture fixture").unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(marker).unwrap(), "call\n");
    fs::remove_dir_all(root).unwrap();
}
