//! Transaction/codec assertions only; these are not real-tool execution evidence.
use super::*;
use arc_ipc::{JobClass, Locality, Reason, Uncertainty};

fn script_request(n: u32, class: JobClass, reason: Reason) -> Request {
    Request::Enqueue {
        job_id: id(n),
        observer: identity(),
        lineage_context: None,
        classification: Classification {
            classification: class,
            reason,
            locality: Locality::HostLocal,
            uncertainty: Uncertainty::None,
        },
    }
}
fn submit(c: &mut Connection, n: u32, class: JobClass, reason: Reason, now: u64) -> Reply {
    enqueue(
        c,
        &script_request(n, class, reason),
        &OwnerEvidence::gather(&identity()),
        &Policy::default(),
        &context(n),
        None,
        now,
    )
    .unwrap()
}
#[test]
fn typecheck_codec_reopen_window_replay_outcome_and_release_are_durable() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let first = submit(
        &mut c,
        1,
        JobClass::Typecheck,
        Reason::BunScriptTypecheck,
        100,
    );
    let generation1 = generation(&first);
    assert_eq!(
        submit(
            &mut c,
            1,
            JobClass::Typecheck,
            Reason::BunScriptTypecheck,
            101
        ),
        first
    );
    assert!(matches!(
        submit(
            &mut c,
            2,
            JobClass::Typecheck,
            Reason::BunScriptTypecheck,
            102
        ),
        Reply::Queued {
            reason: WaitReason::ClassWindow,
            ..
        }
    ));
    let active = active_reservations(&c).unwrap();
    assert_eq!(active.typecheck, 1);
    assert_eq!(active.resources, policy.typecheck.reservation);
    drop(c);
    let mut c = db.open();
    let job = get(&c, &id(1)).unwrap().unwrap();
    assert_eq!(job.classification.classification, JobClass::Typecheck);
    assert_eq!(job.classification.reason, Reason::BunScriptTypecheck);
    assert!(job.is_valid());
    assert!(matches!(
        poll_job(&mut c, 2, 103, &policy),
        Reply::Queued {
            reason: WaitReason::ClassWindow,
            ..
        }
    ));
    let done = complete_job(&mut c, 1, generation1, Outcome::Exited { code: 17 }, 104);
    assert_eq!(
        complete_job(&mut c, 1, generation1, Outcome::Exited { code: 17 }, 105),
        done
    );
    let second = poll_job(&mut c, 2, 106, &policy);
    assert!(generation(&second) > generation1);
    complete_job(
        &mut c,
        2,
        generation(&second),
        Outcome::Exited { code: 0 },
        107,
    );
    assert_eq!(
        active_reservations(&c).unwrap(),
        arc_traffic::Active::default()
    );
    assert_eq!(permit_counts(&c), (2, 0));
    assert_eq!(
        get(&c, &id(1)).unwrap().unwrap().outcome,
        Some(Outcome::Exited { code: 17 })
    );
}
#[test]
fn cargo_and_script_builds_share_durable_class_counter_in_both_orders() {
    for script_first in [false, true] {
        let db = Database::new();
        let mut c = db.open();
        let policy = Policy::default();
        let first = if script_first {
            submit(&mut c, 1, JobClass::Build, Reason::BunScriptBuild, 100)
        } else {
            add(&mut c, 1, "BUILD", 100, &policy)
        };
        let second = if script_first {
            add(&mut c, 2, "BUILD", 101, &policy)
        } else {
            submit(&mut c, 2, JobClass::Build, Reason::BunScriptBuild, 101)
        };
        assert!(matches!(
            second,
            Reply::Queued {
                reason: WaitReason::ClassWindow,
                ..
            }
        ));
        assert_eq!(active_reservations(&c).unwrap().build, 1);
        complete_job(
            &mut c,
            1,
            generation(&first),
            Outcome::Exited { code: 0 },
            102,
        );
        let next = poll_job(&mut c, 2, 103, &policy);
        complete_job(
            &mut c,
            2,
            generation(&next),
            Outcome::Exited { code: 0 },
            104,
        );
        assert_eq!(
            active_reservations(&c).unwrap(),
            arc_traffic::Active::default()
        );
        assert_eq!(permit_counts(&c), (2, 0));
    }
}

mod accounting;
mod diagnostics;
mod scheduling;
