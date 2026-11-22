//! Clock-driven admission deadlines: no scheduler or socket timing assumptions.
use super::*;
use std::cell::{Cell, RefCell};

#[test]
fn native_busy_deadline_is_fixed_until_a_healthy_queue_renews_it() {
    let id = JobId::try_from("a".repeat(32)).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let classification = arc_detector::classify_argv(&["go".into(), "test".into()]);
    let initial = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification,
        lineage_context: None,
        project: None,
        event: None,
        usage: None,
    };
    let continuity = Continuity::default();
    let peer = AdmissionPeer {
        socket: Path::new("unused"),
        id: &id,
        observer: &observer,
        continuity: &continuity,
        route: Route::NativeTools,
    };
    for (queued, healthy) in [(false, false), (true, false), (true, true)] {
        let origin = Instant::now();
        let original = origin + Duration::from_millis(500);
        let clock = Cell::new(origin);
        let deadlines = RefCell::new(Vec::new());
        let calls = Cell::new(0usize);
        let transport = |request: &GovernorJobRequest, deadline: Instant, committed: &mut bool| {
            let now = clock.get();
            let GovernorJobRequest::Traffic { request } = request else {
                panic!("unexpected governor request");
            };
            let number = calls.get();
            assert!(number < 200, "admission failed to advance its clock");
            calls.set(number + 1);
            if queued && number > 0 {
                assert!(matches!(request, JobRequest::Poll { job_id, .. } if job_id == &id));
            } else {
                assert!(matches!(request, JobRequest::Enqueue { job_id, .. } if job_id == &id));
            }
            deadlines.borrow_mut().push((now, deadline));
            if now >= deadline {
                assert!(!healthy, "healthy queue unexpectedly expired");
                assert!(*committed, "a sent busy request remains possibly committed");
                // Exercise the admission loop's own Busy-at-deadline branch;
                // the transport must not supply the outcome under test.
                return Err(Failure::Busy);
            }
            if queued && (number == 0 || healthy && now < origin + Duration::from_secs(3)) {
                *committed = true;
                return Ok(arc_ipc::governor::decode_job_result(&json!({
                    "state":"TRAFFIC", "result": {
                        "state":"QUEUED", "sequence":1, "queued_at_ms":1,
                        "reason":"CLASS_WINDOW"
                    }
                }))
                .unwrap());
            }
            if healthy {
                return Ok(arc_ipc::governor::decode_job_result(&json!({
                    "state":"TRAFFIC", "result": {
                        "state":"GRANTED", "sequence":1, "queued_at_ms":1,
                        "granted_at_ms":2, "permit_generation":1,
                        "reservation":{"cpu_millis":1000,"memory_bytes":1024},
                        "lineage_context":"b".repeat(64)
                    }
                }))
                .unwrap());
            }
            *committed = true;
            Err(Failure::Busy)
        };
        let now = || clock.get();
        let sleep = |duration| clock.set(clock.get() + duration);
        let result = await_admission_with(
            peer,
            &initial,
            false,
            None,
            original,
            &|| false,
            Runtime {
                timing: Timing {
                    now: &now,
                    sleep: &sleep,
                },
                transport,
            },
        );
        let observed = deadlines.into_inner();
        assert_eq!(observed[0], (origin, original));
        if healthy {
            assert!(matches!(
                result,
                Ok(Admission::Reserved { waited: true, .. })
            ));
            assert!(clock.get() > original + Duration::from_millis(1800));
            for ((previous, _), (next, deadline)) in observed.iter().zip(observed.iter().skip(1)) {
                assert_eq!(*next, *previous + Duration::from_millis(40));
                assert_eq!(*deadline, *next + Duration::from_millis(1800));
            }
        } else {
            assert!(matches!(result, Err(Failure::Uncertain)));
            let expected = if queued {
                origin + Duration::from_millis(1840)
            } else {
                original
            };
            let start = usize::from(queued);
            assert!(observed.len() > start + 2);
            assert!(observed[start..]
                .iter()
                .all(|(_, deadline)| *deadline == expected));
            assert_eq!(observed.last(), Some(&(expected, expected)));
            for pair in observed[start..].windows(2) {
                assert_eq!(pair[1].0, pair[0].0 + Duration::from_millis(20));
            }
            assert_eq!(clock.get(), expected);
        }
    }
}
