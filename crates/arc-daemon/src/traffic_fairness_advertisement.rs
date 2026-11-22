//! The daemon's standing advertisement of project fair queuing (AT-TRF-08).
//!
//! `arcctl exec` may send a project key only to a daemon that advertised
//! `arc_ipc::traffic::fairness::CAPABILITY`. A probe request before every
//! enqueue would change the request sequence that older peers and scripted
//! peers see, so the daemon also advertises without a request: it holds a
//! POSIX write lock on one file beside its socket for as long as it runs.
//! The kernel reports the lock holder to any reader and releases the lock
//! when this process ends, however it ends.
use arc_ipc::traffic::fairness::ADVERTISEMENT;
use std::{
    fs::{File, OpenOptions},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::Path,
};

/// Held for the daemon lifetime. The lock belongs to the process, so this
/// file must be opened once and never through a second descriptor.
pub(super) struct Advertisement {
    _file: File,
}

/// Advertise beside `socket`. Failure only means no client sends a key.
pub(super) fn advertise(socket: &Path) -> Option<Advertisement> {
    let directory = socket
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(directory.join(ADVERTISEMENT))
        .ok()?;
    let metadata = file.metadata().ok()?;
    // SAFETY: geteuid has no preconditions.
    if !metadata.is_file() || metadata.uid() != unsafe { libc::geteuid() } {
        return None;
    }
    // SAFETY: an all-zero flock is valid; fcntl receives a live descriptor
    // and a readable lock description covering the whole file.
    let locked = unsafe {
        let mut lock: libc::flock = std::mem::zeroed();
        lock.l_type = libc::F_WRLCK as _;
        lock.l_whence = libc::SEEK_SET as _;
        libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &lock) == 0
    };
    locked.then_some(Advertisement { _file: file })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    struct Directory(PathBuf);
    impl Directory {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("arc-advertisement-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The pid another process sees holding a write lock, or `None`. Only the
    /// child opens the file: a second descriptor closed in this process would
    /// release the very lock under test.
    fn holder_seen_by_another_process(path: &Path) -> Option<u32> {
        use std::os::unix::ffi::OsStrExt;
        let path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        let mut pipe = [0; 2];
        assert_eq!(unsafe { libc::pipe(pipe.as_mut_ptr()) }, 0);
        // SAFETY: the child calls only async-signal-safe functions and exits.
        let child = unsafe { libc::fork() };
        assert!(child >= 0);
        if child == 0 {
            unsafe {
                let fd = libc::open(path.as_ptr(), libc::O_RDONLY);
                let mut lock: libc::flock = std::mem::zeroed();
                lock.l_type = libc::F_RDLCK as _;
                lock.l_whence = libc::SEEK_SET as _;
                let pid: libc::pid_t = if fd >= 0
                    && libc::fcntl(fd, libc::F_GETLK, &mut lock) == 0
                    && lock.l_type != libc::F_UNLCK as libc::c_short
                {
                    lock.l_pid
                } else {
                    0
                };
                libc::write(pipe[1], (&pid as *const libc::pid_t).cast(), 4);
                libc::_exit(0);
            }
        }
        let mut pid: libc::pid_t = 0;
        unsafe {
            libc::close(pipe[1]);
            assert_eq!(
                libc::read(pipe[0], (&mut pid as *mut libc::pid_t).cast(), 4),
                4
            );
            libc::close(pipe[0]);
            let mut status = 0;
            assert_eq!(libc::waitpid(child, &mut status, 0), child);
        }
        (pid != 0).then_some(pid as u32)
    }

    #[test]
    fn the_lock_names_this_process_until_the_advertisement_is_dropped() {
        let directory = Directory::new("held");
        let socket = directory.0.join("arc.sock");
        let path = directory.0.join(ADVERTISEMENT);
        let advertisement = advertise(&socket).expect("advertised");
        let metadata = fs::metadata(&path).unwrap();
        assert_eq!((metadata.mode() & 0o7777, metadata.len()), (0o600, 0));
        assert_eq!(
            holder_seen_by_another_process(&path),
            Some(std::process::id())
        );
        drop(advertisement);
        assert_eq!(holder_seen_by_another_process(&path), None);
        // A file left by an earlier daemon is reused, not replaced.
        let inode = metadata.ino();
        let again = advertise(&socket).expect("advertised again");
        assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
        assert_eq!(
            holder_seen_by_another_process(&path),
            Some(std::process::id())
        );
        drop(again);
    }

    #[test]
    fn an_unusable_location_advertises_nothing() {
        let directory = Directory::new("unusable");
        assert!(advertise(Path::new("arc.sock")).is_none());
        assert!(advertise(&directory.0.join("missing").join("arc.sock")).is_none());
        // A directory or a link where the file belongs.
        let blocked = directory.0.join("blocked");
        fs::create_dir_all(blocked.join(ADVERTISEMENT)).unwrap();
        assert!(advertise(&blocked.join("arc.sock")).is_none());
        let linked = directory.0.join("linked");
        fs::create_dir_all(&linked).unwrap();
        fs::write(linked.join("target"), "").unwrap();
        std::os::unix::fs::symlink(linked.join("target"), linked.join(ADVERTISEMENT)).unwrap();
        assert!(advertise(&linked.join("arc.sock")).is_none());
    }
}
