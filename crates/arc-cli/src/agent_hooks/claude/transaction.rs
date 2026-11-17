//! Serialize ARC writers and reject settings snapshots changed by other writers.
use super::metadata;
use std::{
    ffi::CString,
    fs::{self, File, Metadata, OpenOptions},
    io::{self, Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawFd,
    },
    path::{Path, PathBuf},
};

pub(super) struct Lock(File);

impl Lock {
    pub fn acquire(path: &Path, on_contention: impl FnOnce()) -> Result<Self, &'static str> {
        let parent = path.parent().ok_or("Invalid Claude settings path.")?;
        fs::create_dir_all(parent).map_err(|_| "Cannot create Claude settings directory.")?;
        let mut name = path.as_os_str().to_os_string();
        name.push(".arc-claude-lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(PathBuf::from(name))
            .map_err(|_| "Cannot open Claude settings lock.")?;
        loop {
            // SAFETY: the file descriptor is open and owned by this guard.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                return Ok(Self(file));
            }
            match io::Error::last_os_error().kind() {
                io::ErrorKind::Interrupted => continue,
                io::ErrorKind::WouldBlock => break,
                _ => return Err("Cannot lock Claude settings."),
            }
        }
        on_contention();
        loop {
            // SAFETY: the file descriptor is open and owned by this guard.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
                return Ok(Self(file));
            }
            if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                return Err("Cannot lock Claude settings.");
            }
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        // SAFETY: the descriptor remains open until after this call.
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

pub(super) struct Snapshot {
    file: Option<File>,
    metadata: Option<Metadata>,
    fingerprint: Option<metadata::Fingerprint>,
    pub bytes: Option<Vec<u8>>,
}

impl Snapshot {
    pub fn read(path: &Path) -> Result<Self, &'static str> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Self {
                    file: None,
                    metadata: None,
                    fingerprint: None,
                    bytes: None,
                });
            }
            Err(_) => return Err("Cannot read Claude settings."),
        };
        let before = file
            .metadata()
            .map_err(|_| "Cannot inspect Claude settings.")?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|_| "Cannot read Claude settings.")?;
        let fingerprint = metadata::fingerprint(&file)
            .map_err(|_| "Cannot inspect Claude settings access metadata.")?;
        let after = file
            .metadata()
            .map_err(|_| "Cannot inspect Claude settings.")?;
        if !same_metadata(&before, &after) {
            return Err(STALE);
        }
        Ok(Self {
            file: Some(file),
            metadata: Some(after),
            fingerprint: Some(fingerprint),
            bytes: Some(bytes),
        })
    }

    pub fn ensure_current(&self, path: &Path) -> Result<(), &'static str> {
        let current = Self::read(path)?;
        if self.matches(&current) {
            Ok(())
        } else {
            Err(STALE)
        }
    }

    fn matches(&self, current: &Self) -> bool {
        match (&self.metadata, &current.metadata) {
            (None, None) => true,
            (Some(original), Some(now))
                if same_metadata(original, now)
                    && self.bytes == current.bytes
                    && self.fingerprint == current.fingerprint =>
            {
                true
            }
            _ => false,
        }
    }

    // Exchange can itself advance ctime; compare the moved inode and bytes.
    fn matches_moved(&self, current: &Self) -> bool {
        match (&self.metadata, &current.metadata) {
            (Some(original), Some(now)) => {
                same_moved_metadata(original, now)
                    && self.bytes == current.bytes
                    && self.fingerprint == current.fingerprint
            }
            (None, None) => true,
            _ => false,
        }
    }

    pub fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), &'static str> {
        self.write_with(path, bytes, || {})
    }

    pub fn write_with(
        &self,
        path: &Path,
        bytes: &[u8],
        before_replace: impl FnOnce(),
    ) -> Result<(), &'static str> {
        let parent = path.parent().ok_or("Invalid Claude settings path.")?;
        let id = arc_core::process_identity::random_job_id()
            .map_err(|_| "Cannot allocate a Claude settings temporary file.")?;
        let temporary = parent.join(format!(".arc-claude-{id}.tmp"));
        let mut preserve_temporary = false;
        let result = (|| -> Result<(), &'static str> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)
                .map_err(|_| "Cannot write Claude settings.")?;
            file.write_all(bytes)
                .and_then(|()| file.flush())
                .map_err(|_| "Cannot write Claude settings.")?;
            if let Some(original) = &self.file {
                metadata::preserve(original, &file)
                    .map_err(|_| "Cannot preserve Claude settings metadata.")?;
            }
            // The lock coordinates ARC processes only. Re-read immediately before
            // replacement to detect edits from writers that do not honor it. The
            // exchange also checks the inode actually displaced by replacement.
            let prepared = Self::read(&temporary)?;
            before_replace();
            // Compare after the checkpoint and immediately before replacement.
            // A settings editor need not use ARC's lock.
            self.ensure_current(path)?;
            if self.file.is_none() {
                return fs::rename(&temporary, path)
                    .map_err(|_| "Cannot replace Claude settings safely.");
            }
            swap(&temporary, path).map_err(|error| {
                if error.kind() == io::ErrorKind::NotFound {
                    STALE
                } else {
                    "Cannot replace Claude settings safely."
                }
            })?;
            let displaced = Self::read(&temporary);
            if displaced
                .as_ref()
                .is_ok_and(|current| self.matches_moved(current))
            {
                return fs::remove_file(&temporary)
                    .map_err(|_| "Cannot finish Claude settings update.");
            }
            // Put the displaced edit back if our prepared inode is still at
            // the settings path. Never discard either side on a conflict.
            let installed = Self::read(path);
            if !installed
                .as_ref()
                .is_ok_and(|current| prepared.matches_moved(current))
                || swap(&temporary, path).is_err()
            {
                preserve_temporary = true;
                return Err(CONFLICT);
            }
            if !Self::read(&temporary).is_ok_and(|current| prepared.matches_moved(&current)) {
                preserve_temporary = true;
                return Err(CONFLICT);
            }
            Err(STALE)
        })();
        if result.is_err() && !preserve_temporary {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

const STALE: &str =
    "Claude settings changed during the update; review the current settings and retry.";
const CONFLICT: &str = "Claude settings changed during recovery; review the current settings and retained .arc-claude-*.tmp file before retrying.";

fn swap(left: &Path, right: &Path) -> io::Result<()> {
    let left = CString::new(left.as_os_str().as_bytes())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let right = CString::new(right.as_os_str().as_bytes())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    #[cfg(target_os = "macos")]
    let result = {
        unsafe extern "C" {
            fn renamex_np(from: *const libc::c_char, to: *const libc::c_char, flags: u32) -> i32;
        }
        // SAFETY: both paths are live NUL-terminated strings; 2 is RENAME_SWAP.
        unsafe { renamex_np(left.as_ptr(), right.as_ptr(), 2) }
    };
    #[cfg(target_os = "linux")]
    let result = {
        // SAFETY: both paths are live NUL-terminated strings.
        unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                left.as_ptr(),
                libc::AT_FDCWD,
                right.as_ptr(),
                libc::RENAME_EXCHANGE,
            )
        }
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let result = return Err(io::Error::from(io::ErrorKind::Unsupported));
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn same_metadata(left: &Metadata, right: &Metadata) -> bool {
    same_moved_metadata(left, right)
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

fn same_moved_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
}
