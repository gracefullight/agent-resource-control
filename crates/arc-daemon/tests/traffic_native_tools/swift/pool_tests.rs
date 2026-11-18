use super::*;
use std::sync::mpsc;

fn fixture() -> (PathBuf, Arc<Pool>) {
    let root = std::env::temp_dir().join(format!(
        "arc-swift-pool-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let mut slots = Vec::new();
    for form in [Form::SwiftBuild, Form::SwiftTest] {
        let project = root.join(form.id());
        fs::create_dir_all(project.join("scratch path/plugins/outputs/package")).unwrap();
        fs::write(project.join("scratch path/prepared-product"), b"prepared").unwrap();
        slots.push(Slot {
            form,
            project,
            borrower: None,
            quarantined: false,
        });
    }
    (
        root,
        Arc::new(Pool {
            slots: Mutex::new(slots),
            available: Condvar::new(),
        }),
    )
}

#[test]
fn leased_tree_is_exclusive_and_scenario_state_is_reset_before_reuse() {
    let (root, pool) = fixture();
    let first = pool.lease(Form::SwiftBuild, &root.join("first"));
    fs::write(first.project.join("launches"), b"first").unwrap();
    let evidence = first
        .project
        .join("scratch path/plugins/outputs/package/arc-swift-evidence");
    fs::create_dir_all(&evidence).unwrap();
    fs::write(evidence.join("entry.json"), b"first").unwrap();
    fs::write(
        evidence.parent().unwrap().join("ARCGenerated.swift"),
        b"first",
    )
    .unwrap();
    let path = first.project.clone();
    let (started, waiting) = mpsc::channel();
    let (acquired, result) = mpsc::channel();
    let worker_pool = Arc::clone(&pool);
    let destination = root.join("second");
    let worker = std::thread::spawn(move || {
        started.send(()).unwrap();
        let second = worker_pool.lease(Form::SwiftBuild, &destination);
        assert!(!second.project.join("launches").exists());
        assert!(!second
            .project
            .join("scratch path/plugins/outputs/package/arc-swift-evidence")
            .exists());
        assert!(!second
            .project
            .join("scratch path/plugins/outputs/package/ARCGenerated.swift")
            .exists());
        assert_eq!(
            fs::read(second.project.join("scratch path/prepared-product")).unwrap(),
            b"prepared"
        );
        acquired.send(second.project.clone()).unwrap();
    });
    waiting.recv_timeout(Duration::from_secs(60)).unwrap();
    assert!(matches!(result.try_recv(), Err(mpsc::TryRecvError::Empty)));
    drop(first);
    assert_eq!(result.recv_timeout(Duration::from_secs(60)).unwrap(), path);
    worker.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn specific_returned_tree_waits_without_blocking_another_slot() {
    let (root, pool) = fixture();
    let other_path = root.join("other-build");
    fs::create_dir_all(&other_path).unwrap();
    pool.slots.lock().unwrap().push(Slot {
        form: Form::SwiftBuild,
        project: other_path.clone(),
        borrower: None,
        quarantined: false,
    });
    let first = pool.lease(Form::SwiftBuild, &root.join("first"));
    let original = first.project.clone();
    let expected = original.clone();
    let worker_pool = Arc::clone(&pool);
    let destination = root.join("reacquired");
    let (started, waiting) = mpsc::channel();
    let (acquired, result) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        started.send(()).unwrap();
        let again = worker_pool.lease_matching(Form::SwiftBuild, &destination, Some(&expected));
        acquired.send(again.project.clone()).unwrap();
    });
    waiting.recv_timeout(Duration::from_secs(60)).unwrap();
    assert!(matches!(result.try_recv(), Err(mpsc::TryRecvError::Empty)));
    let independent = pool.lease(Form::SwiftBuild, &root.join("independent"));
    assert_eq!(independent.project, other_path);
    drop(first);
    assert_eq!(
        result.recv_timeout(Duration::from_secs(60)).unwrap(),
        original
    );
    worker.join().unwrap();
    drop(independent);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn late_pair_failure_preserves_both_scenario_diagnostics() {
    let (root, pool) = fixture();
    let first_destination = root.join("pair/first");
    let second_destination = root.join("pair/second");
    let first = pool.lease(Form::SwiftBuild, &first_destination);
    let second = pool.lease(Form::SwiftTest, &second_destination);
    let evidence = first
        .project
        .join("scratch path/plugins/outputs/package/arc-swift-evidence");
    fs::create_dir_all(&evidence).unwrap();
    fs::write(evidence.join("entry.json"), b"first native entry").unwrap();
    fs::write(first.project.join("stdout"), b"first native output").unwrap();
    fs::write(second.project.join("stderr"), b"second native output").unwrap();
    // Even an individually completed first project must survive a later pair
    // reconciliation or shutdown assertion failure.
    cleanup::mark_success(&first.project);
    assert!(std::panic::catch_unwind(|| {
        let _leases = Leases {
            held: Mutex::new(vec![first, second]),
        };
        panic!("simulated failure after both terminal outcomes");
    })
    .is_err());
    assert_eq!(
        fs::read(first_destination.join("stdout")).unwrap(),
        b"first native output"
    );
    assert_eq!(
        fs::read(second_destination.join("stderr")).unwrap(),
        b"second native output"
    );
    assert_eq!(
        fs::read(
            first_destination.join("failure-plugin-outputs/package/arc-swift-evidence/entry.json")
        )
        .unwrap(),
        b"first native entry"
    );
    let slots = pool.slots.lock().unwrap();
    assert!(slots
        .iter()
        .all(|slot| slot.borrower.is_none() && !slot.quarantined));
    assert!(slots
        .iter()
        .all(|slot| slot.project.join("scratch path/prepared-product").is_file()));
    drop(slots);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn completed_independent_case_is_removed_when_a_sibling_fails() {
    let (root, pool) = fixture();
    let success_destination = root.join("offline/success");
    let failure_destination = root.join("offline/failure");
    let success = pool.lease(Form::SwiftBuild, &success_destination);
    let failure = pool.lease(Form::SwiftTest, &failure_destination);
    fs::write(success.project.join("stdout"), b"completed").unwrap();
    fs::write(failure.project.join("stderr"), b"failure evidence").unwrap();
    let success_path = success.project.clone();
    assert!(std::panic::catch_unwind(|| {
        let leases = Leases {
            held: Mutex::new(vec![success, failure]),
        };
        leases.release_success(&success_path);
        panic!("independent sibling failed");
    })
    .is_err());
    assert!(!success_destination.exists());
    assert_eq!(
        fs::read(failure_destination.join("stderr")).unwrap(),
        b"failure evidence"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn completed_comparison_releases_slot_and_retains_evidence_on_later_failure() {
    let (root, pool) = fixture();
    let destination = root.join("managed/completed");
    let completed = pool.lease(Form::SwiftBuild, &destination);
    let completed_path = completed.project.clone();
    fs::write(completed.project.join("stdout"), b"completed comparison").unwrap();
    let result = std::panic::catch_unwind(|| {
        let leases = Leases {
            held: Mutex::new(vec![completed]),
        };
        leases.release_baseline(&completed_path);
        let next = pool.lease(Form::SwiftBuild, &root.join("other"));
        assert_eq!(next.project, completed_path);
        drop(next);
        panic!("later comparison failed");
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(destination.join("stdout")).unwrap(),
        b"completed comparison"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reserved_reuse_slots_do_not_block_other_available_slots() {
    let (root, pool) = fixture();
    for form in [Form::SwiftBuild, Form::SwiftTest] {
        let project = root.join(format!("{}-other", form.id()));
        fs::create_dir_all(&project).unwrap();
        pool.slots.lock().unwrap().push(Slot {
            form,
            project,
            borrower: None,
            quarantined: false,
        });
    }
    let reserved_build = pool.lease(Form::SwiftBuild, &root.join("reuse-build"));
    let reserved_test = pool.lease(Form::SwiftTest, &root.join("reuse-test"));
    let other_pool = Arc::clone(&pool);
    let other_root = root.clone();
    let (completed, observed) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let build = other_pool.lease(Form::SwiftBuild, &other_root.join("independent-build"));
        let test = other_pool.lease(Form::SwiftTest, &other_root.join("independent-test"));
        completed
            .send((build.project.clone(), test.project.clone()))
            .unwrap();
    });
    let (build, test) = observed.recv_timeout(Duration::from_secs(60)).unwrap();
    assert_ne!(build, reserved_build.project);
    assert_ne!(test, reserved_test.project);
    worker.join().unwrap();
    drop((reserved_build, reserved_test));
    fs::remove_dir_all(root).unwrap();
}
