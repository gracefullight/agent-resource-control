use super::*;
use std::{
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::PermissionsExt,
    },
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        Arc,
    },
};

fn queued_with_stderr(broken: bool) {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let project = projects(&f.root, if broken { "broken" } else { "full" });
    let mut first = spawn_build(&f, &project[0], true);
    eventually(|| project[0].join("started").exists());

    let cargo = f.root.join("cargo");
    fs::write(&cargo, "#!/bin/sh\nprintf 'once\\n' >> \"$TRAFFIC_CHILD_MARKER\"\nprintf 'child-output\\n'\nexit 17\n").unwrap();
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o700)).unwrap();
    let mut fds = [-1; 2];
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    let mut reader = Some(unsafe { File::from_raw_fd(fds[0]) });
    let writer = unsafe { File::from_raw_fd(fds[1]) };
    let flags = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETFL) };
    if !broken {
        assert_eq!(
            unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );
        let bytes = [b'x'; 4096];
        loop {
            let n = unsafe { libc::write(writer.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) };
            if n < 0 {
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
    }
    let watch = writer.try_clone().unwrap();
    // Darwin can expose transient kernel-internal bits while a writer sleeps.
    // Compare the user-settable status flags, including O_NONBLOCK.
    let status_mask =
        libc::O_ACCMODE | libc::O_NONBLOCK | libc::O_APPEND | libc::O_ASYNC | libc::O_SYNC;
    let mut sibling = writer.try_clone().unwrap();
    let sibling_done = Arc::new(AtomicBool::new(false));
    let sibling_signal = sibling_done.clone();
    let sibling_writer = (!broken).then(|| {
        thread::spawn(move || {
            let result = sibling.write_all(b"y");
            sibling_signal.store(true, Ordering::SeqCst);
            result
        })
    });
    let stop = Arc::new(AtomicBool::new(false));
    let changed = Arc::new(AtomicBool::new(false));
    let observed_flags = Arc::new(AtomicI32::new(flags));
    let worker_stop = stop.clone();
    let worker_changed = changed.clone();
    let worker_flags = observed_flags.clone();
    let watcher = thread::spawn(move || {
        while !worker_stop.load(Ordering::SeqCst) {
            let actual = unsafe { libc::fcntl(watch.as_raw_fd(), libc::F_GETFL) };
            if actual < 0 || actual & status_mask != flags & status_mask {
                worker_flags.store(actual, Ordering::SeqCst);
                worker_changed.store(true, Ordering::SeqCst);
            }
        }
    });
    if broken {
        drop(reader.take());
    }
    let marker = f.root.join("quiet-child");
    let mut second = OwnedChild::new(
        Command::new(binary("arcctl"))
            .args(["exec", "--", "cargo", "build"])
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env(
                "PATH",
                format!("{}:{}", f.root.display(), std::env::var("PATH").unwrap()),
            )
            .env("TRAFFIC_CHILD_MARKER", &marker)
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdin(Stdio::null())
            .stdout(Stdio::from(
                File::create(f.root.join("quiet-output")).unwrap(),
            ))
            .stderr(Stdio::from(writer))
            .spawn()
            .unwrap(),
    );
    eventually(|| entry(&f, second.child.id()).is_some_and(|e| e.0 == "QUEUED"));
    thread::sleep(Duration::from_millis(150));
    assert!(!marker.exists());
    assert!(
        !changed.load(Ordering::SeqCst),
        "flags {flags:#x} changed to {:#x}",
        observed_flags.load(Ordering::SeqCst)
    );
    if !broken {
        assert!(!sibling_done.load(Ordering::SeqCst));
    }
    fs::write(project[0].join("release"), "release").unwrap();
    finish(&mut first);
    eventually(|| second.child.try_wait().unwrap().is_some());
    assert_eq!(second.child.wait().unwrap().code(), Some(17));
    assert_eq!(fs::read_to_string(marker).unwrap(), "once\n");
    assert_eq!(
        fs::read_to_string(f.root.join("quiet-output")).unwrap(),
        "child-output\n"
    );
    stop.store(true, Ordering::SeqCst);
    watcher.join().unwrap();
    assert!(!changed.load(Ordering::SeqCst));
    if let Some(mut reader) = reader {
        let mut space = [0u8; 4096];
        assert!(reader.read(&mut space).unwrap() > 0);
        sibling_writer.unwrap().join().unwrap().unwrap();
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert!(!bytes.is_empty());
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn queued_child_survives_full_shared_stderr_without_changing_flags() {
    queued_with_stderr(false);
}

#[test]
fn queued_child_survives_broken_stderr_without_changing_flags() {
    queued_with_stderr(true);
}

#[test]
fn killed_queued_wrapper_does_not_leave_notice_pipe_open() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let project = projects(&f.root, "notice_orphan");
    let mut first = spawn_build(&f, &project[0], true);
    eventually(|| project[0].join("started").exists());

    let mut fds = [-1; 2];
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    let reader = unsafe { File::from_raw_fd(fds[0]) };
    let writer = unsafe { File::from_raw_fd(fds[1]) };
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
    let mut wrapper = BuildChild::new(
        OwnedChild::new(
            Command::new(binary("arcctl"))
                .args(["exec", "--", "cargo", "build"])
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
    let deadline = Instant::now() + Duration::from_secs(10);
    let helper = loop {
        if let Some(pid) = children(wrapper.child.id()).first().copied() {
            assert_eq!(
                unsafe { libc::kill(wrapper.child.id() as i32, libc::SIGSTOP) },
                0
            );
            if children(wrapper.child.id()).contains(&pid) {
                break pid;
            }
            assert_eq!(
                unsafe { libc::kill(wrapper.child.id() as i32, libc::SIGCONT) },
                0
            );
        }
        assert!(Instant::now() < deadline, "notice helper never appeared");
        thread::yield_now();
    };
    wrapper.child.kill().unwrap();
    wrapper.child.wait().unwrap();

    // Do not drain the pipe: the helper must time out even while its write blocks.
    let deadline = Instant::now() + Duration::from_secs(3);
    let closed = loop {
        let mut fd = libc::pollfd {
            fd: reader.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert!(unsafe { libc::poll(&mut fd, 1, 0) } >= 0);
        if fd.revents & libc::POLLHUP != 0 {
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        thread::sleep(Duration::from_millis(10));
    };
    if !closed {
        // The helper was observed as this test's own child before wrapper death.
        unsafe { libc::kill(helper as i32, libc::SIGKILL) };
    }
    fs::write(project[0].join("release"), "release").unwrap();
    finish(&mut first);
    daemon.stop(libc::SIGTERM);
    f.report();
    assert!(closed, "orphaned notice helper kept stderr open");
}

#[test]
fn queued_cli_waits_through_real_sqlite_busy_then_starts_after_release() {
    use arc_ipc::{
        jobs::{JobId, ProcessIdentity},
        traffic::JobRequest,
    };
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let projects = projects(&f.root, "busy");
    let mut first = spawn_build(&f, &projects[0], true);
    eventually(|| projects[0].join("started").exists());
    let mut second = spawn_build(&f, &projects[1], true);
    eventually(|| entry(&f, second.child.id()).is_some_and(|e| e.0 == "QUEUED"));
    let lock = database(&f);
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    let probe = JobRequest::Enqueue {
        job_id: JobId::try_from("d123456789abcdef0123456789abcdef".to_owned()).unwrap(),
        observer: ProcessIdentity::capture(std::process::id()).unwrap(),
        classification: cargo_class("BUILD", "CARGO_BUILD"),
        lineage_context: None,
    };
    for _ in 0..4 {
        let response = traffic(&f, &probe);
        assert_eq!(response.error.unwrap().code, arc_ipc::ERROR_DB_BUSY);
    }
    thread::sleep(Duration::from_millis(350));
    assert!(second.child.try_wait().unwrap().is_none());
    assert!(!projects[1].join("started").exists());
    lock.execute_batch("ROLLBACK").unwrap();
    drop(lock);
    thread::sleep(Duration::from_millis(150));
    assert!(entry(&f, second.child.id()).is_some_and(|e| e.0 == "QUEUED"));
    assert_eq!(active(&f).1, 1);
    assert!(!projects[1].join("started").exists());
    fs::write(projects[0].join("release"), "release").unwrap();
    finish(&mut first);
    eventually(|| projects[1].join("started").exists());
    fs::write(projects[1].join("release"), "release").unwrap();
    finish(&mut second);
    assert_eq!(invocations(&projects[0]), 1);
    assert_eq!(invocations(&projects[1]), 1);
    assert_eq!(active(&f).1, 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}
