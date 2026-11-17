//! Wait for a fixture's published log marker or its child's actual exit.
use super::{Fixture, OwnedChild};
use std::{
    fs,
    time::{Duration, Instant},
};

/// Terminate a direct fixture child and observe its exit without an unbounded wait.
pub fn kill_and_reap(child: &mut std::process::Child, owned_group: bool) -> std::io::Result<()> {
    if owned_group && unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) } != 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error);
        }
    }
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    if let Err(error) = child.kill() {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        // On Darwin an exited child can briefly stop accepting signals before
        // its terminal status becomes observable to try_wait.
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error);
        }
    }
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        if Instant::now() >= end {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "fixture child {} did not exit after termination",
                    child.id()
                ),
            ));
        }
        std::thread::yield_now();
    }
}

/// Reap the direct child with native resource usage and a diagnostic watchdog.
#[allow(dead_code)] // Shared fixture targets do not all run Meter calibration.
pub fn wait4_usage(child: &mut std::process::Child) -> std::io::Result<(i32, libc::rusage)> {
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        let mut status = 0;
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        let result =
            unsafe { libc::wait4(child.id() as i32, &mut status, libc::WNOHANG, &mut usage) };
        if result == child.id() as i32 {
            return Ok((status, usage));
        }
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
        if Instant::now() >= end {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("fixture child {} never exited for wait4", child.id()),
            ));
        }
        std::thread::yield_now();
    }
}

fn observed(fixture: &Fixture, child: &mut OwnedChild, marker: &str) -> bool {
    observed_after_read(fixture, child, marker, |_| {})
}

pub fn observed_after_read(
    fixture: &Fixture,
    child: &mut OwnedChild,
    marker: &str,
    after_read: impl FnOnce(&mut OwnedChild),
) -> bool {
    let stdout = fs::read_to_string(fixture.root.join("stdout")).unwrap();
    if stdout.contains(marker) {
        return true;
    }
    after_read(child);
    if let Some(status) = child.child.try_wait().unwrap() {
        child.reaped = true;
        // Publication and exit can occur between the first read and try_wait.
        // Once the child is reaped, this is the final contents of its log.
        let stdout = fs::read_to_string(fixture.root.join("stdout")).unwrap();
        if stdout.contains(marker) {
            return true;
        }
        panic!(
            "child exited before {marker}: {status}; {stdout} {}",
            fs::read_to_string(fixture.root.join("stderr")).unwrap()
        );
    }
    false
}

#[cfg(target_os = "macos")]
pub(super) fn wait_for(fixture: &Fixture, child: &mut OwnedChild, marker: &str) {
    wait_for_with_timeout(fixture, child, marker, Duration::from_secs(60));
}

#[cfg(target_os = "macos")]
pub fn wait_for_with_timeout(
    fixture: &Fixture,
    child: &mut OwnedChild,
    marker: &str,
    timeout: Duration,
) {
    use std::{
        io,
        os::fd::{AsRawFd, FromRawFd, OwnedFd},
        ptr,
    };

    let stdout = fs::File::open(fixture.root.join("stdout")).unwrap();
    let fd = unsafe { libc::kqueue() };
    assert!(fd >= 0, "kqueue: {}", io::Error::last_os_error());
    let queue = unsafe { OwnedFd::from_raw_fd(fd) };
    assert_eq!(
        unsafe { libc::fcntl(queue.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) },
        0,
        "kqueue close-on-exec: {}",
        io::Error::last_os_error()
    );
    let register = |ident: usize, filter: i16, fflags: u32| {
        let mut change: libc::kevent = unsafe { std::mem::zeroed() };
        change.ident = ident as _;
        change.filter = filter;
        change.flags = libc::EV_ADD | libc::EV_ENABLE | libc::EV_CLEAR;
        change.fflags = fflags;
        let result = unsafe {
            libc::kevent(
                queue.as_raw_fd(),
                &change,
                1,
                ptr::null_mut(),
                0,
                ptr::null(),
            )
        };
        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    };
    register(
        stdout.as_raw_fd() as usize,
        libc::EVFILT_VNODE,
        libc::NOTE_WRITE | libc::NOTE_EXTEND | libc::NOTE_ATTRIB,
    )
    .unwrap();
    if let Err(error) = register(
        child.child.id() as usize,
        libc::EVFILT_PROC,
        libc::NOTE_EXIT,
    ) {
        // A very short-lived child can exit before the kernel registers it.
        // Its final log and wait status, rather than elapsed time, decide.
        if observed(fixture, child, marker) {
            return;
        }
        panic!("register fixture child exit: {error}");
    }
    let end = Instant::now() + timeout;
    loop {
        if observed(fixture, child, marker) {
            return;
        }
        let remaining = end.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "fixture child never published marker {marker}"
        );
        let timeout = libc::timespec {
            tv_sec: remaining.as_secs() as _,
            tv_nsec: remaining.subsec_nanos() as _,
        };
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        let count =
            unsafe { libc::kevent(queue.as_raw_fd(), ptr::null(), 0, &mut event, 1, &timeout) };
        if count < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            panic!("wait for fixture marker: {error}");
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn wait_for(fixture: &Fixture, child: &mut OwnedChild, marker: &str) {
    wait_for_with_timeout(fixture, child, marker, Duration::from_secs(60));
}

#[cfg(not(target_os = "macos"))]
pub fn wait_for_with_timeout(
    fixture: &Fixture,
    child: &mut OwnedChild,
    marker: &str,
    timeout: Duration,
) {
    let end = Instant::now() + timeout;
    while !observed(fixture, child, marker) {
        assert!(
            Instant::now() < end,
            "fixture child never published marker {marker}"
        );
        std::thread::yield_now();
    }
}
