use super::*;

// Pipe readiness and native process fork/exit notifications wake the owner;
// no fixed polling interval is part of a successful fixture run.
pub(super) struct OutputEvents {
    queue: OwnedFd,
    watched: Vec<ProcessIdentity>,
}
impl OutputEvents {
    pub(super) fn new(out: Option<&impl AsRawFd>, err: Option<&impl AsRawFd>) -> io::Result<Self> {
        let fd = unsafe { libc::kqueue() };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut events = Self {
            queue: unsafe { OwnedFd::from_raw_fd(fd) },
            watched: Vec::new(),
        };
        for fd in [out.map(AsRawFd::as_raw_fd), err.map(AsRawFd::as_raw_fd)]
            .into_iter()
            .flatten()
        {
            events.register(fd as usize, libc::EVFILT_READ, 0)?;
        }
        Ok(events)
    }

    fn register(&mut self, id: usize, filter: i16, flags: u32) -> io::Result<()> {
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        event.ident = id as _;
        event.filter = filter;
        event.flags = libc::EV_ADD | libc::EV_ENABLE | libc::EV_CLEAR;
        event.fflags = flags;
        if unsafe {
            libc::kevent(
                self.queue.as_raw_fd(),
                &event,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn watch(&mut self, identities: &[ProcessIdentity]) -> io::Result<bool> {
        let mut pending = false;
        for identity in identities {
            if self.watched.contains(identity) {
                continue;
            }
            if let Err(error) = self.register(
                identity.pid as usize,
                libc::EVFILT_PROC,
                libc::NOTE_FORK | libc::NOTE_EXIT,
            ) {
                // A process can exit between identity capture and registration.
                // The next root/pipe event still drives observation and cleanup.
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
                }
                if crate::cleanup::capture(identity.pid)?.as_ref() == Some(identity) {
                    pending = true;
                }
                continue;
            }
            self.watched.push(identity.clone());
        }
        Ok(pending)
    }

    pub(super) fn wait(&self, deadline: Instant) -> io::Result<()> {
        let duration = deadline.saturating_duration_since(Instant::now());
        let timeout = libc::timespec {
            tv_sec: duration.as_secs() as _,
            tv_nsec: duration.subsec_nanos() as _,
        };
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::kevent(
                self.queue.as_raw_fd(),
                std::ptr::null(),
                0,
                &mut event,
                1,
                &timeout,
            )
        };
        if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

pub(super) fn nonblocking(pipe: Option<&impl AsRawFd>) -> io::Result<()> {
    if let Some(pipe) = pipe {
        let fd = pipe.as_raw_fd();
        // SAFETY: only this fixture's captured output descriptors are changed.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
pub(super) fn drain(pipe: &mut Option<impl Read>, bytes: &mut Vec<u8>) -> io::Result<()> {
    let mut buffer = [0; 16384];
    for _ in 0..16 {
        let Some(reader) = pipe.as_mut() else {
            break;
        };
        match reader.read(&mut buffer) {
            Ok(0) => {
                *pipe = None;
                break;
            }
            Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
