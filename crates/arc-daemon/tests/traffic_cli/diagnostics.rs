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

fn queued_cancellation(broken: bool) {
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

fn spawn_failure(broken: bool) {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let (_reader, writer) = stderr_pipe(broken);
    let mut wrapper = OwnedChild::new(
        Command::new(binary("arcctl"))
            .args(["exec", "--", "cargo", "build"])
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
    queued_cancellation(false);
}
#[test]
fn cancellation_with_broken_stderr_exits_and_cleans_up() {
    queued_cancellation(true);
}
#[test]
fn spawn_failure_with_full_stderr_exits_and_releases() {
    spawn_failure(false);
}
#[test]
fn spawn_failure_with_broken_stderr_exits_and_releases() {
    spawn_failure(true);
}
