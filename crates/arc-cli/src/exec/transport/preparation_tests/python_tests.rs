//! Python admission negotiation, isolated from process-wide signal handlers.
use super::*;

#[test]
fn python_negotiation_distinguishes_unsupported_malformed_and_uncertain_admission() {
    use crate::exec::{
        governor,
        signals::Signals,
        traffic::{Mode, Route},
    };
    use std::{fs, os::unix::fs::DirBuilderExt, rc::Rc};
    const NAME: &str = "exec::transport::preparation_tests::python_tests::python_negotiation_distinguishes_unsupported_malformed_and_uncertain_admission";
    if std::env::var("ARC_PYTHON_PREPARATION_CASE").as_deref() != Ok(NAME) {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", NAME, "--nocapture"])
            .env("ARC_PYTHON_PREPARATION_CASE", NAME)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|s| s == "PYTHON_PREPARATION_EXECUTED"));
        return;
    }
    let root = arc_core::runtime_root().unwrap().join(format!(
        "python-prep-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let signals = Signals::install(true).unwrap();
    let id = JobId::try_from("a".repeat(32)).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let classification =
        arc_detector::classify_argv(&[".venv/bin/python".into(), "-m".into(), "pytest".into()]);
    for mode in [
        "probe-unsupported",
        "job-unsupported",
        "uncertain",
        "bad-probe",
        "array-probe",
        "bad-job",
    ] {
        // A lost ACK intentionally retains its witness. Give every independent
        // scenario its own private directory; never erase evidence to reuse an ID.
        let case_root = root.join(mode);
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&case_root)
            .unwrap();
        let socket = case_root.join("unused");
        let original = deadline();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let recorded = seen.clone();
        EXCHANGE.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move |op, payload, end, _| {
                recorded.borrow_mut().push((op.to_owned(), payload.clone()));
                if payload["request"]["action"] == "CANCEL" {
                    return Ok(serde_json::json!({"state":"TRAFFIC", "result":{"state":"CANCELLED"}}));
                }
                assert_eq!(end, original, "Python fallback renewed the admission deadline");
                match op {
                    arc_ipc::traffic::OPERATION_PYTHON_TESTS_CAPABILITIES => {
                        assert!(payload.is_null());
                        if mode == "probe-unsupported" { Err(Failure::Unsupported) }
                        else if mode == "array-probe" { Ok(serde_json::json!([arc_ipc::traffic::PYTHON_TESTS_CAPABILITY])) }
                        else { Ok(serde_json::json!({"capability":if mode == "bad-probe" { "wrong" } else { arc_ipc::traffic::PYTHON_TESTS_CAPABILITY }})) }
                    }
                    arc_ipc::traffic::OPERATION_PYTHON_TESTS_JOB => {
                        assert_eq!(payload["request"]["job_id"], "a".repeat(32));
                        assert_eq!(payload["request"]["classification"]["reason"], "PYTHON_PYTEST");
                        match mode {
                            "job-unsupported" => Err(Failure::Unsupported),
                            "bad-job" => Ok(serde_json::json!({"state":"FUTURE"})),
                            "uncertain" => Err(Failure::Uncertain),
                            _ => panic!("unexpected Python admission"),
                        }
                    }
                    "job.start" => {
                        assert!(matches!(mode, "probe-unsupported" | "job-unsupported"));
                        assert_eq!(payload["job_id"], "a".repeat(32));
                        assert_eq!(payload["action"], "REGISTER");
                        assert_eq!(payload["classification"], serde_json::json!(classification.legacy()));
                        Err(Failure::Uncertain)
                    }
                    _ => panic!("unexpected operation {op}"),
                }
            }));
        });
        let _guard = ScriptedExchangeGuard;
        let result = governor::prepare(&socket, &id, &observer, classification, original, &signals);
        match mode {
            "uncertain" => {
                assert!(matches!(
                    result,
                    Ok(Mode::Uncertain {
                        route: Route::PythonTests,
                        ..
                    })
                ));
                result.unwrap().finish_unspawned(&socket, &id, &observer);
            }
            "probe-unsupported" | "job-unsupported" => {
                assert!(matches!(result, Ok(Mode::Legacy(Tracking::Uncertain))))
            }
            _ => assert!(matches!(result, Err(Failure::InvalidReply)), "{mode}"),
        }
        let seen = seen.borrow();
        assert_eq!(
            seen[0].0,
            arc_ipc::traffic::OPERATION_PYTHON_TESTS_CAPABILITIES
        );
        assert_eq!(
            seen.iter().filter(|(op, _)| op == "job.start").count(),
            usize::from(matches!(mode, "probe-unsupported" | "job-unsupported"))
        );
        assert_eq!(
            seen.iter()
                .filter(|(op, p)| op == arc_ipc::traffic::OPERATION_PYTHON_TESTS_JOB
                    && p["request"]["action"] == "ENQUEUE")
                .count(),
            usize::from(!matches!(
                mode,
                "probe-unsupported" | "bad-probe" | "array-probe"
            ))
        );
    }
    drop(signals);
    fs::remove_dir_all(root).unwrap();
    println!("PYTHON_PREPARATION_EXECUTED");
}
