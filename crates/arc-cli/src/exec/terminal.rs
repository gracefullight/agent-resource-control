//! A controlling terminal identifies a shell job, even while it is backgrounded.
//! ARC never takes foreground ownership from the caller or its pipeline peers.
pub(crate) fn shares_job() -> bool {
    if [0, 1, 2]
        .into_iter()
        .any(|fd| unsafe { libc::tcgetpgrp(fd) } > 0)
    {
        return true;
    }
    let fd = unsafe { libc::open(c"/dev/tty".as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
    if fd < 0 {
        return false;
    }
    let controlling = unsafe { libc::tcgetpgrp(fd) } > 0;
    unsafe { libc::close(fd) };
    controlling
}
