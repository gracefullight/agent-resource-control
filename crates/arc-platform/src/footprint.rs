//! Native reads of a job footprint (lane B: AT-MTR-02, G-01 item 4).
//!
//! [`working_directory`] reads the current directory of a verified ARC
//! process (the job's observer), revalidating its identity before and after
//! the read so a recycled PID is never measured. [`allocated_bytes`] walks
//! the tree under a directory and sums `st_blocks * 512` of every entry
//! (directories, files and symlinks themselves): it never follows a symlink,
//! never descends into a directory on another device, counts a hard-linked
//! inode once, and stops at its entry and time bounds. The result is
//! complete, partial (a lower bound: a bound was hit or an entry could not
//! be read) or unavailable (the root itself could not be read). Nothing read
//! here is logged or stored except the byte total.
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use std::{
    collections::HashSet,
    ffi::{CStr, CString},
    fs, io,
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd, RawFd},
        unix::ffi::OsStrExt,
    },
    path::{Path, PathBuf},
    time::Instant,
};

/// Bytes per `st_blocks` unit (POSIX).
pub const BLOCK_BYTES: u64 = 512;
/// Entries one walk reads at most.
pub const MAX_ENTRIES: u64 = 200_000;

/// One allocated-bytes reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Footprint {
    /// Every entry under the root on its device was counted.
    Complete(u64),
    /// A lower bound: a bound was hit or some entry could not be read.
    Partial(u64),
    /// The root could not be read.
    Unavailable,
}

/// The bounds of one walk.
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub max_entries: u64,
    pub deadline: Instant,
}

/// Allocated bytes under `root`, within `bounds`.
pub fn allocated_bytes(root: &Path, bounds: Bounds) -> Footprint {
    walk(root, bounds, |_| {})
}

// Each stream owns its directory descriptor. Child opens are relative to that
// descriptor and must match the identity that was just read by fstatat.
struct Directory(*mut libc::DIR);

impl Directory {
    fn new(file: fs::File) -> io::Result<Self> {
        let fd = file.into_raw_fd();
        // SAFETY: fd is an owned directory descriptor; success transfers it.
        let stream = unsafe { libc::fdopendir(fd) };
        if stream.is_null() {
            let error = io::Error::last_os_error();
            // SAFETY: fdopendir failed, so ownership was not transferred.
            unsafe { libc::close(fd) };
            Err(error)
        } else {
            Ok(Self(stream))
        }
    }

    fn fd(&self) -> RawFd {
        // SAFETY: the stream is live until Drop.
        unsafe { libc::dirfd(self.0) }
    }

    fn next(&mut self) -> io::Result<Option<CString>> {
        clear_errno();
        // SAFETY: this stream is exclusively borrowed and remains live.
        let entry = unsafe { libc::readdir(self.0) };
        if entry.is_null() {
            let error = io::Error::last_os_error();
            return if error.raw_os_error() == Some(0) {
                Ok(None)
            } else {
                Err(error)
            };
        }
        // SAFETY: readdir returns a NUL-terminated name valid until next call.
        Ok(Some(
            unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_owned(),
        ))
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        // SAFETY: this uniquely owned stream was returned by fdopendir.
        unsafe { libc::closedir(self.0) };
    }
}

#[cfg(any(target_vendor = "apple", target_os = "freebsd"))]
fn clear_errno() {
    // SAFETY: errno is thread-local.
    unsafe { *libc::__error() = 0 };
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "hurd",
    target_os = "redox",
    target_os = "emscripten"
))]
fn clear_errno() {
    // SAFETY: errno is thread-local.
    unsafe { *libc::__errno_location() = 0 };
}

#[cfg(any(target_os = "android", target_os = "netbsd", target_os = "openbsd"))]
fn clear_errno() {
    // SAFETY: errno is thread-local.
    unsafe { *libc::__errno() = 0 };
}

#[cfg(any(target_os = "solaris", target_os = "illumos"))]
fn clear_errno() {
    // SAFETY: errno is thread-local.
    unsafe { *libc::___errno() = 0 };
}

#[cfg(target_os = "aix")]
fn clear_errno() {
    // SAFETY: errno is thread-local.
    unsafe { *libc::_Errno() = 0 };
}

// On another Unix, an old errno may conservatively mark a completed walk
// partial; it can never make an unreadable directory look complete.
#[cfg(not(any(
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "hurd",
    target_os = "redox",
    target_os = "emscripten",
    target_os = "android",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "solaris",
    target_os = "illumos",
    target_os = "aix"
)))]
fn clear_errno() {}

fn open_directory(parent: RawFd, name: &CStr) -> io::Result<fs::File> {
    // SAFETY: name is NUL terminated and parent remains open during openat.
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: fd is a newly owned descriptor.
        Ok(unsafe { fs::File::from_raw_fd(fd) })
    }
}

fn stat_at(parent: RawFd, name: &CStr) -> io::Result<libc::stat> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: stat is writable and fstatat does not follow a symlink.
    if unsafe {
        libc::fstatat(
            parent,
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a successful fstatat initialized the structure.
    Ok(unsafe { stat.assume_init() })
}

fn stat_fd(fd: RawFd) -> io::Result<libc::stat> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fd is live and stat is writable.
    if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a successful fstat initialized the structure.
    Ok(unsafe { stat.assume_init() })
}

fn walk(root: &Path, bounds: Bounds, mut before_open: impl FnMut(&CStr)) -> Footprint {
    let Ok(root_name) = CString::new(root.as_os_str().as_bytes()) else {
        return Footprint::Unavailable;
    };
    let Ok(root_file) = open_directory(libc::AT_FDCWD, &root_name) else {
        return Footprint::Unavailable;
    };
    let Ok(top) = stat_fd(root_file.as_raw_fd()) else {
        return Footprint::Unavailable;
    };
    let device = top.st_dev;
    let mut total = (top.st_blocks.max(0) as u64).saturating_mul(BLOCK_BYTES);
    let mut entries = 1u64;
    let mut partial = false;
    let mut linked = HashSet::new();
    let Ok(root_stream) = Directory::new(root_file) else {
        return Footprint::Unavailable;
    };
    let mut stack = vec![root_stream];
    while let Some(directory) = stack.last_mut() {
        if entries >= bounds.max_entries || Instant::now() >= bounds.deadline {
            partial = true;
            break;
        }
        let name = match directory.next() {
            Ok(Some(name)) => name,
            Ok(None) => {
                stack.pop();
                continue;
            }
            Err(_) => {
                partial = true;
                stack.pop();
                continue;
            }
        };
        if matches!(name.to_bytes(), b"." | b"..") {
            continue;
        }
        let Ok(metadata) = stat_at(directory.fd(), &name) else {
            partial = true;
            continue;
        };
        entries += 1;
        let child_device = metadata.st_dev;
        if child_device != device {
            continue;
        }
        let is_directory = metadata.st_mode & libc::S_IFMT == libc::S_IFDIR;
        if metadata.st_nlink > 1 && !is_directory && !linked.insert((child_device, metadata.st_ino))
        {
            continue;
        }
        total =
            total.saturating_add((metadata.st_blocks.max(0) as u64).saturating_mul(BLOCK_BYTES));
        if is_directory {
            before_open(&name);
            let opened = open_directory(directory.fd(), &name).and_then(|file| {
                let found = stat_fd(file.as_raw_fd())?;
                if found.st_dev != device || found.st_ino != metadata.st_ino {
                    return Err(io::Error::other("directory changed during traversal"));
                }
                Directory::new(file)
            });
            match opened {
                Ok(child) => stack.push(child),
                Err(_) => partial = true,
            }
        }
    }
    if partial {
        Footprint::Partial(total)
    } else {
        Footprint::Complete(total)
    }
}

/// The current directory of `identity`, only while that very process is
/// alive before and after the read.
pub fn working_directory(identity: &ProcessIdentity) -> Option<PathBuf> {
    if probe_live(identity) != IdentityStatus::VerifiedMatch {
        return None;
    }
    let directory = native_cwd(identity.pid)?;
    (probe_live(identity) == IdentityStatus::VerifiedMatch).then_some(directory)
}

#[cfg(target_os = "macos")]
fn native_cwd(pid: u32) -> Option<PathBuf> {
    use std::{ffi::CStr, os::unix::ffi::OsStrExt};
    let pid = libc::c_int::try_from(pid).ok()?;
    // SAFETY: a zeroed plain-data struct is a valid output buffer.
    let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
    let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_vnodepathinfo>()).ok()?;
    // SAFETY: `info` is writable for `size` bytes.
    let read = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            (&mut info as *mut libc::proc_vnodepathinfo).cast(),
            size,
        )
    };
    if read != size {
        return None;
    }
    // SAFETY: `vip_path` is MAXPATHLEN contiguous `c_char`s.
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            info.pvi_cdir.vip_path.as_ptr().cast::<u8>(),
            std::mem::size_of_val(&info.pvi_cdir.vip_path),
        )
    };
    let path = CStr::from_bytes_until_nul(bytes).ok()?.to_bytes();
    (!path.is_empty() && path[0] == b'/').then(|| PathBuf::from(std::ffi::OsStr::from_bytes(path)))
}

#[cfg(not(target_os = "macos"))]
fn native_cwd(pid: u32) -> Option<PathBuf> {
    fs::read_link(format!("/proc/{pid}/cwd"))
        .ok()
        .filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        os::unix::fs::{symlink, MetadataExt},
        time::Duration,
    };

    fn bounds() -> Bounds {
        Bounds {
            max_entries: MAX_ENTRIES,
            deadline: Instant::now() + Duration::from_secs(60),
        }
    }

    fn scratch(name: &str) -> PathBuf {
        // The workspace target directory is ignored by Git, unlike a
        // crate-relative one.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("footprint-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write(path: &Path, bytes: usize) {
        let mut file = fs::File::create(path).unwrap();
        file.write_all(&vec![7u8; bytes]).unwrap();
        file.sync_all().unwrap();
    }

    #[test]
    fn allocated_bytes_count_blocks_once_and_never_follow_a_symlink() {
        let root = scratch("walk");
        let empty = match allocated_bytes(&root, bounds()) {
            Footprint::Complete(bytes) => bytes,
            other => panic!("{other:?}"),
        };
        fs::create_dir(root.join("nested")).unwrap();
        write(&root.join("nested/data"), 1 << 20);
        let file_blocks = fs::symlink_metadata(root.join("nested/data"))
            .unwrap()
            .blocks()
            * BLOCK_BYTES;
        assert!(file_blocks >= 1 << 20);
        let before = match allocated_bytes(&root, bounds()) {
            Footprint::Complete(bytes) => bytes,
            other => panic!("{other:?}"),
        };
        assert!(before >= empty + file_blocks);
        // A hard link adds no blocks; a symlink to a large outside file adds
        // only its own (small) inode.
        fs::hard_link(root.join("nested/data"), root.join("linked")).unwrap();
        let outside = scratch("outside");
        write(&outside.join("big"), 4 << 20);
        symlink(&outside, root.join("away")).unwrap();
        let after = match allocated_bytes(&root, bounds()) {
            Footprint::Complete(bytes) => bytes,
            other => panic!("{other:?}"),
        };
        assert!(
            after >= before && after < before + (1 << 20),
            "{after} {before}"
        );
        // Bounds turn the reading into a lower bound, never a zero.
        let tight = Bounds {
            max_entries: 2,
            deadline: Instant::now() + Duration::from_secs(60),
        };
        assert!(matches!(allocated_bytes(&root, tight), Footprint::Partial(n) if n <= after));
        let expired = Bounds {
            max_entries: MAX_ENTRIES,
            deadline: Instant::now(),
        };
        assert!(matches!(
            allocated_bytes(&root, expired),
            Footprint::Partial(_)
        ));
        // A missing root or a file root is unavailable.
        assert_eq!(
            allocated_bytes(&root.join("absent"), bounds()),
            Footprint::Unavailable
        );
        assert_eq!(
            allocated_bytes(&root.join("linked"), bounds()),
            Footprint::Unavailable
        );
        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }

    #[test]
    fn replacing_discovered_directory_with_symlink_never_descends_outside_root() {
        let root = scratch("replacement");
        let outside = scratch("replacement-outside");
        fs::create_dir(root.join("nested")).unwrap();
        write(&outside.join("large"), 4 << 20);
        let mut replaced = false;
        let reading = walk(&root, bounds(), |name| {
            if name.to_bytes() == b"nested" && !replaced {
                fs::rename(root.join("nested"), outside.join("original")).unwrap();
                symlink(&outside, root.join("nested")).unwrap();
                replaced = true;
            }
        });
        assert!(replaced);
        assert!(
            matches!(reading, Footprint::Partial(bytes) if bytes < 4 << 20),
            "{reading:?}"
        );
        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }

    #[test]
    fn the_working_directory_is_read_only_for_the_verified_process() {
        let me = ProcessIdentity::capture(std::process::id()).unwrap();
        let read = working_directory(&me).unwrap();
        assert_eq!(
            fs::canonicalize(read).unwrap(),
            fs::canonicalize(std::env::current_dir().unwrap()).unwrap()
        );
        let mut recycled = me.clone();
        recycled.start_token.push('0');
        assert_eq!(working_directory(&recycled), None);
    }
}
