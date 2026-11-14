//! Controlled accounting workload used by the deterministic CLI tests.
use std::{
    fs::OpenOptions,
    io::Write,
    process::Command,
    time::{Duration, Instant},
};

pub fn run(mode: &str) {
    if mode == "pipe-parent" {
        pipe_parent();
        return;
    }
    if let Some(fd) = std::env::var_os("ARC_FIXTURE_CANCEL_FD") {
        use std::{io::Read, os::fd::FromRawFd};
        let fd = fd.to_str().unwrap().parse::<i32>().unwrap();
        // SAFETY: spawn_with_descendants installs this fixture-owned inherited
        // descriptor before spawning the wrapper. This process owns its copy.
        let mut cancel = unsafe { std::fs::File::from_raw_fd(fd) };
        std::thread::spawn(move || loop {
            match cancel.read(&mut [0u8]) {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                _ => unsafe { libc::_exit(125) },
            }
        });
    }
    if mode == "pipe-holder" {
        use std::os::fd::FromRawFd;
        let fd = std::env::var("ARC_FIXTURE_PIPE_READY_FD")
            .unwrap()
            .parse()
            .unwrap();
        // The cancellation reader is installed before acknowledging existence.
        // No ownership identity is published until the test adopts the holder.
        let mut ready = unsafe { std::fs::File::from_raw_fd(fd) };
        ready.write_all(b"r").unwrap();
        drop(ready);
        loop {
            std::thread::park();
        }
    }
    if let Some(ready) = std::env::var_os("ARC_FIXTURE_BEFORE_IDENTITY") {
        // A cancellation/unwind checkpoint before registration, marker writes,
        // workload and SIGSTOP. The inherited cancel channel already owns it.
        std::fs::write(ready, std::process::id().to_string()).unwrap();
        let end = Instant::now() + Duration::from_secs(60);
        loop {
            assert!(Instant::now() < end, "pre-publication fixture watchdog");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    if let Some(root) = std::env::var_os("ARC_FIXTURE_OWNERS") {
        let identity =
            arc_core::process_identity::ProcessIdentity::capture(std::process::id()).unwrap();
        let root = std::path::Path::new(&root);
        std::fs::create_dir_all(root).unwrap();
        publish(root, &identity);
    }
    if let Some(marker) = std::env::var_os("ARC_METER_TEST_MARKER") {
        writeln!(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(marker)
                .unwrap(),
            "{}",
            std::process::id()
        )
        .unwrap();
    }
    if let Some(ready) = std::env::var_os("ARC_METER_TEST_READY") {
        std::fs::write(ready, std::process::id().to_string()).unwrap();
    }
    match mode {
        "hold" => {
            let release = std::env::var_os("ARC_METER_TEST_RELEASE").unwrap();
            let deadline = Instant::now() + Duration::from_secs(60);
            while !std::path::Path::new(&release).exists() {
                assert!(Instant::now() < deadline, "owned hold fixture timed out");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        "escaped" => {
            // A bounded owned descendant outlives its parent. Record identity
            // before exit so acceptance can prove it is missing from wait4.
            let mut child = Command::new(std::env::current_exe().unwrap())
                .env("ARC_FIXTURE_ACCOUNTING", "orphan-work")
                .env_remove("ARC_METER_TEST_MARKER")
                .spawn()
                .unwrap();
            let identity =
                arc_core::process_identity::ProcessIdentity::capture(child.id()).unwrap();
            if let Some(root) = std::env::var_os("ARC_FIXTURE_OWNERS") {
                publish(std::path::Path::new(&root), &identity);
            }
            std::fs::write(
                std::env::var_os("ARC_METER_TEST_ESCAPED").unwrap(),
                serde_json::to_vec(&identity).unwrap(),
            )
            .unwrap();
            // The fixture owner records this identity before releasing it.
            // An explicit release keeps liveness independent of host speed.
            let _ = child.try_wait().unwrap();
            return;
        }
        "orphan-work" => {
            let release = std::env::var_os("ARC_METER_TEST_ESCAPE_RELEASE").unwrap();
            let start = Instant::now();
            let end = start + Duration::from_secs(60);
            // Keep the original CPU workload (at least 400ms), and hold the
            // descendant's lifetime until the observer explicitly releases it.
            while !std::path::Path::new(&release).exists()
                || start.elapsed() < Duration::from_millis(400)
            {
                assert!(Instant::now() < end, "escaped fixture watchdog");
                std::hint::black_box(start.elapsed());
            }
            return;
        }
        "nested" | "nested-scheduled" => {
            let mut child = Command::new(std::env::var_os("ARC_METER_TEST_CLI").unwrap());
            child.args(["exec", "--"]);
            if mode == "nested-scheduled" {
                child.args(["cargo", "build"]);
            } else {
                child.arg(std::env::current_exe().unwrap());
            }
            let status = child
                .env("ARC_FIXTURE_ACCOUNTING", "work")
                .status()
                .unwrap();
            std::process::exit(status.code().unwrap_or(1));
        }
        "stop" => {
            assert_eq!(unsafe { libc::raise(libc::SIGSTOP) }, 0);
        }
        "signal" => {
            unsafe { libc::raise(libc::SIGTERM) };
            unreachable!();
        }
        "quick" => std::process::exit(17),
        _ => (),
    }
    let start = Instant::now();
    let mut work = 0u64;
    while start.elapsed() < Duration::from_millis(120) {
        for _ in 0..1000 {
            work = std::hint::black_box(work.wrapping_mul(17).wrapping_add(1));
        }
    }
    std::hint::black_box(work);
    std::process::exit(if mode == "success" { 0 } else { 17 });
}

/// The parent owns its Child before exposing the pre-adoption checkpoint. On
/// cancellation it kills AND waits for that child before closing its lifetime
/// token. Normal exit deliberately leaves the holder to the external owner.
fn pipe_parent() {
    use std::{
        io::Read,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{net::UnixStream, process::CommandExt},
        },
        process::Stdio,
    };
    struct Holder(
        std::process::Child,
        std::path::PathBuf,
        Option<arc_core::process_identity::ProcessIdentity>,
    );
    impl Drop for Holder {
        fn drop(&mut self) {
            let identity = self
                .2
                .clone()
                .or_else(|| arc_core::process_identity::ProcessIdentity::capture(self.0.id()).ok());
            let _ = self.0.kill();
            let reaped = self.0.wait().is_ok();
            // Reap before fallible receipt I/O. A failed publication must not
            // unwind past the Child owner and leak its pre-adoption descendant.
            let published = identity
                .as_ref()
                .is_some_and(|identity| try_publish(&self.1, identity).is_ok());
            let root = self.1.parent().unwrap();
            if !reaped || !published {
                let _ = std::fs::write(
                    root.join("owned-cleanup-incomplete"),
                    b"pipe holder cleanup unconfirmed",
                );
            }
            let _ = std::fs::write(
                root.join("holder-cleanup"),
                serde_json::to_vec(&serde_json::json!({"identity": identity, "reaped": reaped}))
                    .unwrap(),
            );
        }
    }
    let cancel_fd = std::env::var("ARC_FIXTURE_CANCEL_FD")
        .unwrap()
        .parse()
        .unwrap();
    let cancel = unsafe { std::fs::File::from_raw_fd(cancel_fd) };
    let (mut ready, token) = UnixStream::pair().unwrap();
    let token_fd = token.as_raw_fd();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env("ARC_FIXTURE_ACCOUNTING", "pipe-holder")
        .env("ARC_FIXTURE_PIPE_READY_FD", token_fd.to_string())
        .stdin(Stdio::null());
    // The inherited cancel/lifetime descriptors are already installed by the
    // external owner. This exact readiness descriptor additionally survives exec.
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(token_fd, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let owners = std::path::PathBuf::from(std::env::var_os("ARC_FIXTURE_OWNERS").unwrap());
    let mut holder = Holder(command.spawn().unwrap(), owners, None);
    drop(token);
    let wait = |fd| {
        let end = Instant::now() + Duration::from_secs(60);
        loop {
            let mut fds = [
                libc::pollfd {
                    fd: cancel.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            let remaining = end
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(60000) as i32;
            let n = unsafe { libc::poll(fds.as_mut_ptr(), 2, remaining) };
            if n < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            assert!(n > 0, "pipe fixture readiness/release watchdog");
            if fds[0].revents != 0 {
                return false;
            }
            if fds[1].revents != 0 {
                return true;
            }
        }
    };
    if !wait(ready.as_raw_fd()) {
        return;
    }
    let mut ack = [0u8];
    ready.read_exact(&mut ack).unwrap();
    assert_eq!(ack, *b"r");
    // Internal Child ownership is established already; retain its live identity
    // before cancellation can make native executable identity unavailable.
    holder.2 = Some(arc_core::process_identity::ProcessIdentity::capture(holder.0.id()).unwrap());
    let checkpoint =
        std::path::PathBuf::from(std::env::var_os("ARC_FIXTURE_PIPE_CHECKPOINT").unwrap());
    let pending = checkpoint.with_extension("pending");
    std::fs::write(&pending, b"holder exists; identity not published").unwrap();
    std::fs::rename(pending, &checkpoint).unwrap();
    // The test can now fail or cancel before own_process/registry publication.
    if !wait(libc::STDIN_FILENO) {
        return;
    }
    let mut action = [0u8];
    if std::io::stdin().read(&mut action).unwrap() != 1 || action != *b"a" {
        return;
    }
    let identity = holder.2.as_ref().unwrap();
    publish(&holder.1, identity);
    std::fs::write(
        checkpoint.with_file_name("holder-identity"),
        serde_json::to_vec(identity).unwrap(),
    )
    .unwrap();
    // A distinct release follows adoption; EOF releases the successful case.
    if !wait(libc::STDIN_FILENO) {
        return;
    }
    let _ = std::io::stdin().read(&mut action).unwrap();
    std::io::stdout().write_all(&vec![b'o'; 262144]).unwrap();
    std::io::stderr().write_all(&vec![b'e'; 262144]).unwrap();
    // Intentional exited-parent scenario: the external owner has adopted this
    // exact identity before releasing stdin, and will terminate/reap it.
    std::mem::forget(holder);
    std::process::exit(17);
}

fn publish(root: &std::path::Path, identity: &arc_core::process_identity::ProcessIdentity) {
    try_publish(root, identity).unwrap();
}
fn try_publish(
    root: &std::path::Path,
    identity: &arc_core::process_identity::ProcessIdentity,
) -> std::io::Result<()> {
    let pending = root.join(format!("{}-{}.pending", identity.pid, std::process::id()));
    std::fs::write(
        &pending,
        serde_json::to_vec(identity).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(pending, root.join(format!("{}.json", identity.pid)))
}
