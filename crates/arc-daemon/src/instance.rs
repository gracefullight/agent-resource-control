//! Single-instance lock: OS file lock plus pid and process start time.
//!
//! A live daemon keeps the exclusive lock and answers IPC. A second daemon for
//! the same user exits non-zero. A lock left behind by a dead pid, or whose
//! recorded start time no longer matches the pid, is recovered without killing
//! any other process.

use crate::platform;
use std::{
    error::Error,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum InstanceError {
    Duplicate { anchor: PathBuf },
    Io(io::Error),
}

impl fmt::Display for InstanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate { anchor } => {
                write!(
                    formatter,
                    "socket already served by another daemon for this user; only one daemon per user is allowed (instance anchor: {})",
                    anchor.display()
                )
            }
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for InstanceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Duplicate { .. } => None,
        }
    }
}

impl From<io::Error> for InstanceError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Held for the daemon lifetime; dropping it releases the OS lock.
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    /// Acquire the stable anchor or report that a live daemon owns admission.
    pub fn acquire(path: &Path) -> Result<Self, InstanceError> {
        validate_lock_path(path)?;
        // Serialize acquisition per anchor directory. Two daemons recovering
        // the same stale lock could otherwise both pass the staleness check,
        // and the slower one would unlink the lock file its successor had
        // just created and locked, leaving two live daemons.
        let _serialized = AcquireGuard::lock(path)?;
        for _ in 0..3 {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(path)?;
            validate_lock_metadata(path, &file.metadata()?)?;
            match file.try_lock() {
                Ok(()) => {
                    let owner = Owner {
                        pid: std::process::id(),
                        start_time: platform::process_start_time(std::process::id())?,
                    };
                    write_owner(&file, owner)?;
                    return Ok(Self { _file: file });
                }
                Err(fs::TryLockError::WouldBlock) => {
                    // Judge the inode we opened, not whatever the pathname
                    // names by the time a second open would run.
                    if !read_owner(&file).is_some_and(is_stale) {
                        return Err(InstanceError::Duplicate {
                            anchor: path.to_path_buf(),
                        });
                    }
                    // The pid is dead or the start time changed: replace the
                    // stale lock file and lock the new inode. The previous
                    // holder is left running. Only unlink the pathname while
                    // it still names the inode judged stale above.
                    remove_if_same_inode(path, &file)?;
                }
                Err(fs::TryLockError::Error(error)) => return Err(InstanceError::Io(error)),
            }
        }
        Err(InstanceError::Duplicate {
            anchor: path.to_path_buf(),
        })
    }
}

/// Exclusive OS lock on the lock file's directory, held only while acquiring.
///
/// The directory is never unlinked by the daemon, so every acquirer for the
/// same user contends on one stable inode.
struct AcquireGuard {
    _directory: File,
}

impl AcquireGuard {
    fn lock(lock_path: &Path) -> io::Result<Self> {
        let directory = lock_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let directory = crate::server::open_private_directory(directory)?;
        directory.lock()?;
        Ok(Self {
            _directory: directory,
        })
    }
}

fn is_stale(owner: Owner) -> bool {
    let alive = platform::process_is_alive(owner.pid);
    let same_process = alive
        && platform::process_start_time(owner.pid).map_or(true, |start| start == owner.start_time);
    !same_process
}

/// Unlink `path` only if it still names the inode behind `opened`.
///
/// Returns whether the pathname was removed. A pathname that already names a
/// successor's lock file, or no file at all, is left alone.
fn remove_if_same_inode(path: &Path, opened: &File) -> io::Result<bool> {
    let opened = opened.metadata()?;
    let current = match fs::symlink_metadata(path) {
        Ok(current) => current,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if (current.dev(), current.ino()) != (opened.dev(), opened.ino()) {
        return Ok(false);
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

// Keep the lock pathname across shutdowns. Unlinking it while its inode is
// locked could let another opener lock a different inode for the same user.
// A stale recovered holder must also never unlink its successor's lock file.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Owner {
    pid: u32,
    start_time: u64,
}

pub(crate) fn validate_lock_path(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => validate_lock_metadata(path, &metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn validate_lock_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if !private_lock_attributes(
        metadata.is_file(),
        metadata.uid(),
        metadata.mode(),
        unsafe { libc::geteuid() },
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{} must be an owned regular 0600 instance lock, not a symlink or other entry",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn private_lock_attributes(regular: bool, owner: u32, mode: u32, uid: u32) -> bool {
    regular && owner == uid && mode & 0o7777 == 0o600
}

fn write_owner(mut file: &File, owner: Owner) -> io::Result<()> {
    file.set_len(0)?;
    file.rewind()?;
    write!(file, "pid={}\nstart_time={}\n", owner.pid, owner.start_time)?;
    file.flush()
}

fn read_owner(mut file: &File) -> Option<Owner> {
    let mut contents = String::new();
    file.rewind().ok()?;
    file.read_to_string(&mut contents).ok()?;
    let mut pid = None;
    let mut start_time = None;
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("pid=") {
            pid = value.parse().ok();
        } else if let Some(value) = line.strip_prefix("start_time=") {
            start_time = value.parse().ok();
        }
    }
    Some(Owner {
        pid: pid?,
        start_time: start_time?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::sync::atomic::{AtomicU64, Ordering};

    fn lock_path(path: &Path) -> PathBuf {
        path.to_owned()
    }

    fn scratch(_name: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let directory = arc_core::runtime_root().unwrap().join(format!(
            "i{:x}-{:x}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        directory.join("arc.sock")
    }

    fn cleanup(socket: &Path) {
        let _ = fs::remove_dir_all(socket.parent().unwrap());
    }

    fn hold_lock_with_owner(path: &Path, owner: Owner) -> File {
        fs::write(
            path,
            format!("pid={}\nstart_time={}\n", owner.pid, owner.start_time),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        file.try_lock().unwrap();
        file
    }

    #[test]
    fn a_live_duplicate_is_rejected() {
        let socket = scratch("duplicate");
        let first = InstanceLock::acquire(&socket).unwrap();
        match InstanceLock::acquire(&socket) {
            Err(InstanceError::Duplicate { .. }) => {}
            Err(other) => panic!("expected duplicate, got {other}"),
            Ok(_) => panic!("a second lock must not be granted"),
        }
        drop(first);
        cleanup(&socket);
    }

    #[test]
    fn a_mismatched_start_time_is_recovered_without_killing_anyone() {
        let socket = scratch("mismatch");
        let path = lock_path(&socket);
        let actual = platform::process_start_time(std::process::id()).unwrap();
        let held = hold_lock_with_owner(
            &path,
            Owner {
                pid: std::process::id(),
                start_time: actual.wrapping_add(1),
            },
        );
        let recovered = InstanceLock::acquire(&socket).unwrap();
        assert_eq!(
            read_owner(&File::open(lock_path(&socket)).unwrap()),
            Some(Owner {
                pid: std::process::id(),
                start_time: actual,
            })
        );
        drop(recovered);
        drop(held);
        cleanup(&socket);
    }

    #[test]
    fn a_dead_pid_is_recovered_without_killing_anyone() {
        let socket = scratch("dead");
        let path = lock_path(&socket);
        let held = hold_lock_with_owner(
            &path,
            Owner {
                pid: 1_000_000_000,
                start_time: 1,
            },
        );
        let recovered = InstanceLock::acquire(&socket).unwrap();
        assert_eq!(
            read_owner(&File::open(lock_path(&socket)).unwrap()),
            Some(Owner {
                pid: std::process::id(),
                start_time: platform::process_start_time(std::process::id()).unwrap(),
            })
        );
        drop(recovered);
        drop(held);
        cleanup(&socket);
    }

    #[test]
    fn dropping_a_stale_holder_preserves_its_successors_lock() {
        let socket = scratch("successor");
        let stale = InstanceLock::acquire(&socket).unwrap();
        write_owner(
            &stale._file,
            Owner {
                pid: 1_000_000_000,
                start_time: 1,
            },
        )
        .unwrap();
        let successor = InstanceLock::acquire(&socket).unwrap();
        drop(stale);
        assert!(matches!(
            InstanceLock::acquire(&socket),
            Err(InstanceError::Duplicate { .. })
        ));
        drop(successor);
        cleanup(&socket);
    }

    #[test]
    fn a_stale_inode_never_unlinks_the_successors_lock_file() {
        // Reproduces the losing side of two concurrent recoveries: this
        // acquirer judged inode A stale, but before it unlinked the pathname
        // another recoverer replaced A with its own locked inode B.
        let socket = scratch("replaced");
        let path = lock_path(&socket);
        fs::write(&path, "pid=1000000000\nstart_time=1\n").unwrap();
        let judged_stale = File::open(&path).unwrap();
        fs::remove_file(&path).unwrap();
        let successor = InstanceLock::acquire(&socket).unwrap();

        assert!(!remove_if_same_inode(&path, &judged_stale).unwrap());
        assert_eq!(
            read_owner(&File::open(&path).unwrap()),
            Some(Owner {
                pid: std::process::id(),
                start_time: platform::process_start_time(std::process::id()).unwrap(),
            })
        );
        assert!(matches!(
            InstanceLock::acquire(&socket),
            Err(InstanceError::Duplicate { .. })
        ));
        drop(successor);
        cleanup(&socket);
    }

    #[test]
    fn a_stale_inode_still_named_by_the_path_is_unlinked() {
        let socket = scratch("unchanged");
        let path = lock_path(&socket);
        fs::write(&path, "pid=1000000000\nstart_time=1\n").unwrap();
        let judged_stale = File::open(&path).unwrap();

        assert!(remove_if_same_inode(&path, &judged_stale).unwrap());
        assert!(!path.exists());
        assert!(!remove_if_same_inode(&path, &judged_stale).unwrap());
        cleanup(&socket);
    }

    #[test]
    fn unsafe_anchor_entries_are_rejected_without_repair_or_writes() {
        use std::os::unix::fs::symlink;
        // The sandbox strips setuid on real files; exercise that policy bit
        // directly as well as checking modes the filesystem retains below.
        for mode in [0o644, 0o660, 0o1600, 0o2600, 0o4600] {
            assert!(!private_lock_attributes(true, 42, mode, 42));
        }
        assert!(!private_lock_attributes(true, 42, 0o600, 43));
        assert!(!private_lock_attributes(false, 42, 0o600, 42));
        for mode in [0o644, 0o660, 0o1600] {
            let path = scratch("unsafe");
            fs::write(&path, b"protected owner record").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            let before = fs::symlink_metadata(&path).unwrap();
            assert!(
                InstanceLock::acquire(&path).is_err(),
                "requested mode {mode:o}, actual mode {:o}",
                before.mode() & 0o7777
            );
            let after = fs::symlink_metadata(&path).unwrap();
            assert_eq!(
                (before.ino(), before.uid(), before.mode()),
                (after.ino(), after.uid(), after.mode())
            );
            assert_eq!(fs::read(&path).unwrap(), b"protected owner record");
            cleanup(&path);
        }
        let path = scratch("entry");
        let target = path.with_file_name("target");
        fs::write(&target, b"symlink target").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&target, &path).unwrap();
        assert!(InstanceLock::acquire(&path).is_err());
        assert_eq!(fs::read_link(&path).unwrap(), target);
        assert_eq!(fs::read(&target).unwrap(), b"symlink target");
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(InstanceLock::acquire(&path).is_err());
        assert!(path.is_dir());
        fs::remove_dir(&path).unwrap();
        let name = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        assert!(InstanceLock::acquire(&path).is_err());
        cleanup(&path);
    }
}
