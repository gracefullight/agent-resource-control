//! Scripted peer replies exercise the maintained admission loop, not a native daemon.
mod deadline;
mod live;
use super::super::*;
use crate::exec::{
    governor,
    signals::Signals,
    traffic::{Mode, Route},
};
use arc_ipc::traffic::native::{CAPABILITY, OPERATION_CAPABILITIES, OPERATION_JOB};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::Path,
    process::{Command, ExitCode},
    rc::Rc,
};

#[derive(Clone, Copy)]
enum Ending {
    Granted,
    Cancelled,
    Uncertain,
}

#[derive(Default)]
struct Seen {
    probes: usize,
    enqueue: Option<Value>,
    retry_payload: Option<Value>,
    witness: Option<(u64, Vec<u8>)>,
    busy: usize,
    terminal_replies: usize,
    cancels: usize,
    last_busy: Option<Instant>,
    first_deadline: Option<Instant>,
}

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let root = arc_core::runtime_root().unwrap().join(format!(
            "native-busy-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self(root)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn isolated(name: &str) -> bool {
    if std::env::var("ARC_NATIVE_TOOLS_BUSY_CASE").as_deref() == Ok(name) {
        return false;
    }
    // Process-wide signal handlers must never race other libtest cases. All
    // inherited coverage variables remain intact in this owned subprocess.
    let output = native::OwnedChild::new(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture"])
            .env("ARC_NATIVE_TOOLS_BUSY_CASE", name)
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    )
    .wait_with_output()
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|s| s == format!("{name}:executed")));
    if name.ends_with("cancellation_never_starts_a_child") {
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("NOT_STARTED\nExecution cancelled. No child was started."));
    }
    true
}

fn script(root: &Path, queued: bool, ending: Ending) -> Rc<RefCell<Seen>> {
    let seen = Rc::new(RefCell::new(Seen::default()));
    let state = seen.clone();
    let root = root.to_owned();
    EXCHANGE.with(|slot| {
        assert!(slot
            .borrow_mut()
            .replace(Box::new(move |op, payload, end, cancelled| {
                let mut s = state.borrow_mut();
                assert!(!root.join("child-started").exists());
                if let Some(returned_at) = s.last_busy.take() {
                    // A lower bound only: scheduling delays cannot make a correct
                    // retry fail. No throughput or maximum-latency assertion.
                    assert!(
                        returned_at.elapsed() >= Duration::from_millis(20),
                        "NativeTools skipped the maintained database-busy backoff"
                    );
                }
                if op == OPERATION_CAPABILITIES {
                    assert!(payload.is_null());
                    s.probes += 1;
                    s.first_deadline = Some(end);
                    return Ok(json!({"capability":CAPABILITY}));
                }
                // Any legacy probe, fallback registration, launch or effect replay
                // is an error in these pre-spawn scenarios.
                assert_eq!(op, OPERATION_JOB);
                assert_eq!(payload["action"], "TRAFFIC");
                let request = &payload["request"];
                let action = request["action"].as_str().unwrap();
                if s.enqueue.is_none() {
                    assert_eq!(action, "ENQUEUE");
                    assert_eq!(end, s.first_deadline.unwrap());
                    assert_eq!(request["classification"]["reason"], "GO_TEST");
                    s.enqueue = Some(request.clone());
                }
                let enqueue = s.enqueue.as_ref().unwrap();
                assert_eq!(request["job_id"], enqueue["job_id"]);
                assert_eq!(request["observer"], enqueue["observer"]);
                let id = request["job_id"].as_str().unwrap();
                let path = root.join(format!(".traffic-witness-{id}"));
                let bytes = fs::read(&path).unwrap();
                assert_eq!(bytes.len(), 136);
                assert!(bytes[24..40].iter().all(|b| *b == 0), "no native child yet");
                assert_eq!(
                    &bytes[40..104],
                    enqueue["lineage_context"].as_str().unwrap().as_bytes()
                );
                assert_eq!(&bytes[104..], id.as_bytes());
                assert_eq!(
                    u64::from_be_bytes(bytes[8..16].try_into().unwrap()),
                    request["observer"]["pid"].as_u64().unwrap()
                );
                assert_eq!(
                    u64::from_be_bytes(bytes[16..24].try_into().unwrap()).to_string(),
                    request["observer"]["start_token"].as_str().unwrap()
                );
                let inode = fs::metadata(&path).unwrap().ino();
                if let Some((first_inode, first_bytes)) = &s.witness {
                    assert_eq!(inode, *first_inode, "busy must not recreate the witness");
                    assert_eq!(&bytes[8..], &first_bytes[8..]);
                } else {
                    s.witness = Some((inode, bytes.clone()));
                }
                if action == "CANCEL" {
                    assert_eq!(&bytes[..8], b"ARCNOS03");
                    s.cancels += 1;
                    return Ok(json!({"state":"TRAFFIC","result":{"state":"CANCELLED"}}));
                }
                assert!(!cancelled());
                assert_eq!(&bytes[..8], b"ARCWIT03");
                if queued && action == "ENQUEUE" && s.retry_payload.is_none() {
                    // Once queued, every busy retry must remain POLL, never ENQUEUE.
                    s.retry_payload = Some(json!({
                        "action":"POLL","job_id":request["job_id"],"observer":request["observer"]
                    }));
                    return Ok(json!({"state":"TRAFFIC","result":{
                        "state":"QUEUED","sequence":1,"queued_at_ms":1,"reason":"CLASS_WINDOW"
                    }}));
                }
                let expected = s.retry_payload.get_or_insert_with(|| request.clone());
                assert_eq!(request, expected);
                if s.busy < 3 {
                    s.busy += 1;
                    if s.busy == 3 && matches!(ending, Ending::Cancelled) {
                        // Real pre-spawn cancellation reaches the existing handler.
                        assert_eq!(unsafe { libc::raise(libc::SIGTERM) }, 0);
                        assert!(cancelled());
                    }
                    s.last_busy = Some(Instant::now());
                    return Err(Failure::Busy);
                }
                s.terminal_replies += 1;
                match ending {
                    Ending::Granted => Ok(json!({"state":"TRAFFIC","result":{
                        "state":"GRANTED","sequence":1,"queued_at_ms":1,"granted_at_ms":2,
                        "permit_generation":1,"reservation":{"cpu_millis":1000,"memory_bytes":1024},
                        "lineage_context":s.enqueue.as_ref().unwrap()["lineage_context"]
                    }})),
                    Ending::Uncertain => Err(Failure::Uncertain),
                    Ending::Cancelled => panic!("cancellation retried admission"),
                }
            }))
            .is_none());
    });
    seen
}

fn prepare_case(queued: bool, ending: Ending) {
    let root = Root::new();
    let signals = Signals::install(true).unwrap();
    let seen = script(&root.0, queued, ending);
    let _guard = ScriptedExchangeGuard;
    let id = JobId::try_from("a".repeat(32)).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let classification = arc_detector::classify_argv(&["go".into(), "test".into()]);
    let result = governor::prepare(
        &root.0.join("unused"),
        &id,
        &observer,
        classification,
        crate::exec::preparation::deadline(Instant::now()),
        &signals,
    )
    .unwrap();
    match (&result, ending) {
        (
            Mode::Scheduled {
                route: Route::NativeTools,
                generation: 1,
                waited,
                ..
            },
            Ending::Granted,
        ) => {
            assert_eq!(*waited, queued);
        }
        (
            Mode::Uncertain {
                route: Route::NativeTools,
                ..
            },
            Ending::Uncertain,
        ) => (),
        _ => panic!("busy changed the negotiated admission outcome"),
    }
    let witness = root.0.join(format!(".traffic-witness-{}", id.as_str()));
    assert_eq!(
        fs::read(&witness).unwrap(),
        seen.borrow().witness.as_ref().unwrap().1
    );
    result.finish_unspawned(&root.0.join("unused"), &id, &observer);
    assert_unspawned(&witness, &seen.borrow());
    let seen = seen.borrow();
    assert_eq!(
        (seen.probes, seen.busy, seen.terminal_replies, seen.cancels),
        (1, 3, 1, 1)
    );
    assert!(seen.last_busy.is_none());
}

fn assert_unspawned(path: &Path, seen: &Seen) {
    // The scripted transport supplies replies without authenticated continuity.
    // A reply alone must not erase recovery evidence or authorize another copy.
    let (inode, mut expected) = seen.witness.clone().unwrap();
    expected[..8].copy_from_slice(b"ARCNOS03");
    assert_eq!(fs::metadata(path).unwrap().ino(), inode);
    assert_eq!(fs::read(path).unwrap(), expected);
}

#[test]
fn native_tools_busy_retries_admission_with_one_identity_and_witness() {
    const NAME: &str = "exec::transport::preparation_tests::native_tools::busy::native_tools_busy_retries_admission_with_one_identity_and_witness";
    if isolated(NAME) {
        return;
    }
    for queued in [false, true] {
        prepare_case(queued, Ending::Granted);
    }
    println!("{NAME}:executed");
}

#[test]
fn native_tools_busy_then_uncertain_never_replays_or_falls_back() {
    const NAME: &str = "exec::transport::preparation_tests::native_tools::busy::native_tools_busy_then_uncertain_never_replays_or_falls_back";
    if isolated(NAME) {
        return;
    }
    for queued in [false, true] {
        prepare_case(queued, Ending::Uncertain);
    }
    println!("{NAME}:executed");
}

#[test]
fn native_tools_busy_cancellation_never_starts_a_child() {
    const NAME: &str = "exec::transport::preparation_tests::native_tools::busy::native_tools_busy_cancellation_never_starts_a_child";
    if isolated(NAME) {
        return;
    }
    for queued in [false, true] {
        let root = Root::new();
        let executable = root.0.join("go");
        fs::write(
            &executable,
            b"#!/bin/sh\nprintf ran > \"$ARC_NATIVE_TOOLS_BUSY_MARKER\"\nexit 99\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        std::env::set_var(arc_core::ARC_SOCKET_PATH, root.0.join("unused"));
        std::env::set_var("ARC_NATIVE_TOOLS_BUSY_MARKER", root.0.join("child-started"));
        let seen = script(&root.0, queued, Ending::Cancelled);
        let _guard = ScriptedExchangeGuard;
        assert_eq!(
            crate::exec::run(vec![executable.into_os_string(), "test".into()]),
            ExitCode::FAILURE
        );
        assert!(!root.0.join("child-started").exists());
        let seen = seen.borrow();
        assert_eq!(
            (seen.probes, seen.busy, seen.terminal_replies, seen.cancels),
            (1, 3, 0, 1)
        );
        assert!(seen.last_busy.is_none());
        let id = seen.enqueue.as_ref().unwrap()["job_id"].as_str().unwrap();
        assert_unspawned(&root.0.join(format!(".traffic-witness-{id}")), &seen);
    }
    println!("{NAME}:executed");
}
