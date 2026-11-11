use super::*;
use std::os::fd::{AsRawFd, FromRawFd};

fn stderr_pipe(broken: bool) -> (Option<File>, File) {
    let mut fds = [-1; 2];
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    let reader = unsafe { File::from_raw_fd(fds[0]) };
    let writer = unsafe { File::from_raw_fd(fds[1]) };
    if broken {
        drop(reader);
        return (None, writer);
    }
    let flags = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETFL) };
    assert_eq!(
        unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
        0
    );
    let bytes = [b'x'; 4096];
    loop {
        if unsafe { libc::write(writer.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) } < 0 {
            assert_eq!(
                std::io::Error::last_os_error().kind(),
                std::io::ErrorKind::WouldBlock
            );
            break;
        }
    }
    assert_eq!(
        unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_SETFL, flags) },
        0
    );
    (Some(reader), writer)
}

fn queued_cancellation(broken: bool, program: &str) {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let projects = projects(
        &f.root,
        if broken {
            "cancel_broken"
        } else {
            "cancel_full"
        },
    );
    let mut first = spawn_build(&f, &projects[0], true);
    eventually(|| projects[0].join("started").exists());
    let (_reader, writer) = stderr_pipe(broken);
    let mut second = BuildChild::new(
        OwnedChild::new(
            Command::new(binary("arcctl"))
                .args([
                    "exec",
                    "--",
                    program,
                    if program == "bun" { "test" } else { "build" },
                ])
                .env(arc_core::ARC_SOCKET_PATH, &f.socket)
                .env_remove("ARC_TRAFFIC_LINEAGE")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::from(writer))
                .spawn()
                .unwrap(),
        ),
        &f,
        true,
    );
    eventually(|| entry(&f, second.child.id()).is_some_and(|e| e.0 == "QUEUED"));
    let job_id: String = database(&f)
        .query_row(
            "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1",
            [second.child.id()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        unsafe { libc::kill(second.child.id() as i32, libc::SIGTERM) },
        0
    );
    let limit = Instant::now() + Duration::from_secs(5);
    while second.child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < limit, "cancellation blocked on stderr");
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!second.child.wait().unwrap().success());
    assert!(entry(&f, second.child.id()).is_some_and(|e| e.0 == "CANCELLED"));
    assert!(!f.root.join(format!(".traffic-witness-{job_id}")).exists());
    assert_eq!(active(&f).1, 1);
    assert!(!projects[1].join("started").exists());
    fs::write(projects[0].join("release"), "release").unwrap();
    finish(&mut first);
    daemon.stop(libc::SIGTERM);
    f.report();
}

fn spawn_failure(broken: bool, program: &str) {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let (_reader, writer) = stderr_pipe(broken);
    let mut wrapper = OwnedChild::new(
        Command::new(binary("arcctl"))
            .args([
                "exec",
                "--",
                program,
                if program == "bun" { "test" } else { "build" },
            ])
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env("PATH", &f.root)
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(writer))
            .spawn()
            .unwrap(),
    );
    let limit = Instant::now() + Duration::from_secs(5);
    while wrapper.child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < limit, "spawn failure blocked on stderr");
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!wrapper.child.wait().unwrap().success());
    assert!(entry(&f, wrapper.child.id()).is_some_and(|e| e.0 == "COMPLETED"));
    assert_eq!(active(&f).1, 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn cancellation_with_full_stderr_exits_and_cleans_up() {
    queued_cancellation(false, "cargo");
    queued_cancellation(false, "bun");
}
#[test]
fn cancellation_with_broken_stderr_exits_and_cleans_up() {
    queued_cancellation(true, "cargo");
    queued_cancellation(true, "bun");
}
#[test]
fn spawn_failure_with_full_stderr_exits_and_releases() {
    spawn_failure(false, "cargo");
    spawn_failure(false, "bun");
}
#[test]
fn spawn_failure_with_broken_stderr_exits_and_releases() {
    spawn_failure(true, "cargo");
    spawn_failure(true, "bun");
}

#[test]
fn persisted_mixed_tests_wait_for_independent_cpu_and_memory_budgets() {
    use arc_ipc::{
        jobs::{JobId, ProcessIdentity},
        traffic::{JobRequest, JobResult, LineageContext, WaitReason},
    };
    use arc_storage::jobs::traffic as store;
    let _serial = support::serial();
    for (first_reason, second_reason) in [("CARGO_TEST", "BUN_TEST"), ("BUN_TEST", "CARGO_TEST")] {
        for scarce in [WaitReason::CpuBudget, WaitReason::MemoryBudget] {
            let f = Fixture::new();
            let mut db = arc_storage::open_and_migrate(&f.database).unwrap();
            let mut policy = arc_traffic::Policy::default();
            let reservation = policy.test.reservation;
            policy.test.window = 2;
            policy.build.reservation = reservation;
            policy.budget.cpu_millis =
                reservation.cpu_millis * 2 - u64::from(scarce == WaitReason::CpuBudget);
            policy.budget.memory_bytes =
                reservation.memory_bytes * 2 - u64::from(scarce == WaitReason::MemoryBudget);
            assert_eq!(policy.validate(), Ok(()));
            let observer = ProcessIdentity::capture(std::process::id()).unwrap();
            let owner = store::OwnerEvidence::gather(&observer);
            let id1 = JobId::try_from("1123456789abcdef0123456789abcdef".to_owned()).unwrap();
            let id2 = JobId::try_from("2123456789abcdef0123456789abcdef".to_owned()).unwrap();
            let first = JobRequest::Enqueue {
                job_id: id1.clone(),
                observer: observer.clone(),
                classification: cargo_class("TEST", first_reason),
                lineage_context: None,
            };
            let second = JobRequest::Enqueue {
                job_id: id2.clone(),
                observer: observer.clone(),
                classification: cargo_class("TEST", second_reason),
                lineage_context: None,
            };
            let context1 = LineageContext::try_from("a".repeat(64)).unwrap();
            let context2 = LineageContext::try_from("b".repeat(64)).unwrap();
            let JobResult::Granted {
                permit_generation, ..
            } = store::enqueue(&mut db, &first, &owner, &policy, &context1, None, 100).unwrap()
            else {
                panic!("expected first grant")
            };
            let before = store::active_reservations(&db).unwrap();
            assert_eq!((before.test, before.resources), (1, reservation));
            assert!(
                matches!(store::enqueue(&mut db, &second, &owner, &policy, &context2, None, 101).unwrap(), JobResult::Queued { reason, .. } if reason == scarce)
            );
            assert_eq!(store::active_reservations(&db).unwrap(), before);
            let cancel = JobRequest::Cancel {
                job_id: id1,
                observer: observer.clone(),
                permit_generation: Some(permit_generation),
            };
            assert_eq!(
                store::confirm_cancel_unspawned(&mut db, &cancel, &owner, 102).unwrap(),
                JobResult::Cancelled {}
            );
            let poll = JobRequest::Poll {
                job_id: id2.clone(),
                observer: observer.clone(),
            };
            let JobResult::Granted {
                permit_generation, ..
            } = store::poll(&mut db, &poll, &owner, &policy, &context2, 103).unwrap()
            else {
                panic!("expected grant after release")
            };
            assert_eq!(store::active_reservations(&db).unwrap(), before);
            let cancel = JobRequest::Cancel {
                job_id: id2,
                observer,
                permit_generation: Some(permit_generation),
            };
            assert_eq!(
                store::confirm_cancel_unspawned(&mut db, &cancel, &owner, 104).unwrap(),
                JobResult::Cancelled {}
            );
            assert_eq!(
                store::active_reservations(&db).unwrap(),
                arc_traffic::Active::default()
            );
        }
    }
}

include!("diagnostics/frozen.rs");
include!("diagnostics/legacy.rs");
include!("diagnostics/legacy_recovery.rs");
include!("diagnostics/two_store.rs");
