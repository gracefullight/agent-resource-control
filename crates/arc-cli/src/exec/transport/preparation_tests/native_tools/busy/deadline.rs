//! Full CLI fail-open with a virtual clock and Busy through the exact deadline.
use super::*;
use crate::exec::governor::test_clock;

#[derive(Default)]
struct Seen {
    id: Option<String>,
    observer: Option<Value>,
    witness: Option<(u64, Vec<u8>)>,
    deadline: Option<Instant>,
    before: usize,
    at_boundary: usize,
    queued: bool,
}

#[test]
fn native_tools_sustained_busy_starts_once_only_after_deadline() {
    const NAME: &str = "exec::transport::preparation_tests::native_tools::busy::deadline::native_tools_sustained_busy_starts_once_only_after_deadline";
    if isolated(NAME) {
        return;
    }
    for queued in [false, true] {
        let root = Root::new();
        let executable = root.0.join("go");
        let marker = root.0.join("child-started");
        fs::write(
            &executable,
            b"#!/bin/sh\nprintf 'ran\\n' >> \"$ARC_NATIVE_TOOLS_BUSY_MARKER\"\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        std::env::set_var(arc_core::ARC_SOCKET_PATH, root.0.join("unused"));
        std::env::set_var("ARC_NATIVE_TOOLS_BUSY_MARKER", &marker);
        let _clock = test_clock::install(Instant::now());
        let seen = Rc::new(RefCell::new(Seen::default()));
        let recorded = seen.clone();
        let fixture_root = root.0.clone();
        EXCHANGE.with(|slot| {
            assert!(slot
                .borrow_mut()
                .replace(Box::new(move |op, payload, end, _| {
                    if op == OPERATION_CAPABILITIES {
                        test_clock::set(end - Duration::from_millis(1800));
                        return Ok(json!({"capability":CAPABILITY}));
                    }
                    if op != OPERATION_JOB {
                        return Err(Failure::Uncertain);
                    }
                    let request = &payload["request"];
                    let action = request["action"].as_str().unwrap();
                    if !matches!(action, "ENQUEUE" | "POLL") {
                        return Err(Failure::Uncertain);
                    }
                    assert!(
                        !fixture_root.join("child-started").exists(),
                        "child started before Busy expired"
                    );
                    let mut state = recorded.borrow_mut();
                    let id = request["job_id"].as_str().unwrap();
                    if let Some(original) = &state.id {
                        assert_eq!(id, original);
                        assert_eq!(
                            request["observer"],
                            state.observer.as_ref().unwrap().clone()
                        );
                        assert_eq!(action, if queued { "POLL" } else { "ENQUEUE" });
                    } else {
                        assert_eq!(action, "ENQUEUE");
                        state.id = Some(id.to_owned());
                        state.observer = Some(request["observer"].clone());
                    }
                    let path = fixture_root.join(format!(".traffic-witness-{id}"));
                    let bytes = fs::read(&path).unwrap();
                    assert_eq!(&bytes[..8], b"ARCWIT03");
                    assert!(bytes[24..40].iter().all(|byte| *byte == 0));
                    let witness = (fs::metadata(&path).unwrap().ino(), bytes);
                    if let Some(original) = &state.witness {
                        assert_eq!(&witness, original);
                    } else {
                        state.witness = Some(witness);
                    }
                    if queued && !state.queued {
                        state.queued = true;
                        return Ok(json!({"state":"TRAFFIC","result":{
                            "state":"QUEUED","sequence":1,"queued_at_ms":1,"reason":"CLASS_WINDOW"
                        }}));
                    }
                    let now = test_clock::now();
                    assert!(now <= end);
                    assert_eq!(*state.deadline.get_or_insert(end), end);
                    if now == end {
                        state.at_boundary += 1;
                    } else {
                        state.before += 1;
                    }
                    Err(Failure::Busy)
                }))
                .is_none());
        });
        let _transport = ScriptedExchangeGuard;
        assert_eq!(
            crate::exec::run(vec![executable.into_os_string(), "test".into()]),
            ExitCode::SUCCESS
        );
        assert_eq!(
            fs::read(&marker).unwrap(),
            b"ran\n",
            "exactly one child execution"
        );
        let state = seen.borrow();
        assert!(state.before > 1);
        assert_eq!(
            state.at_boundary, 1,
            "the admission loop must receive Busy at expiry"
        );
        assert_eq!(test_clock::now(), state.deadline.unwrap());
        let (inode, before) = state.witness.as_ref().unwrap();
        let path = root
            .0
            .join(format!(".traffic-witness-{}", state.id.as_ref().unwrap()));
        let after = fs::read(&path).unwrap();
        assert_eq!(fs::metadata(path).unwrap().ino(), *inode);
        assert_eq!(&after[40..], &before[40..]);
        assert!(after[24..40].iter().any(|byte| *byte != 0));
    }
    println!("{NAME}:executed");
}
