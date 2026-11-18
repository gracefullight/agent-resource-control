use super::*;

pub(super) fn wait_for_release_loss(
    fd: std::os::fd::RawFd,
    watchdog: std::time::Duration,
) -> io::Result<()> {
    let deadline = Instant::now() + watchdog;
    let mut observed = libc::pollfd {
        fd,
        events: libc::POLLIN | libc::POLLHUP,
        revents: 0,
    };
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "release socket never closed",
            ));
        }
        let millis = remaining
            .as_millis()
            .saturating_add(1)
            .min(i32::MAX as u128) as i32;
        let ready = unsafe { libc::poll(&mut observed, 1, millis) };
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if ready == 0 {
            continue;
        }
        let mut byte = 0u8;
        let count = unsafe {
            libc::recv(
                fd,
                (&mut byte as *mut u8).cast(),
                1,
                libc::MSG_PEEK | libc::MSG_DONTWAIT,
            )
        };
        if count == 0 {
            return Ok(());
        }
        if count < 0 {
            let error = io::Error::last_os_error();
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) {
                continue;
            }
            return Err(error);
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected release socket data",
        ));
    }
}
