//! ARC installers coordinate using a stable advisory lock beside hooks.json.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawFd,
    },
    path::Path,
};

pub(super) fn acquire(dir: &Path, on_contention: impl FnOnce()) -> Result<File, &'static str> {
    fs::create_dir_all(dir).map_err(|_| "Cannot create Cursor configuration directory.")?;
    let path = dir.join(".hooks.json.arc.lock");
    let open = |create: bool| {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(create)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
    };
    let file = match open(true) {
        Ok(file) => {
            // A restrictive umask must not make the lock we just created
            // inaccessible to the same user on the next invocation.
            // SAFETY: file is an open descriptor created by this call.
            if unsafe { libc::fchmod(file.as_raw_fd(), 0o600) } != 0 {
                return Err("Cannot set Cursor configuration lock permissions.");
            }
            file
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            open(false).map_err(|_| "Cannot open Cursor configuration lock.")?
        }
        Err(_) => return Err("Cannot open Cursor configuration lock."),
    };
    if !file
        .metadata()
        .map_err(|_| "Cannot inspect Cursor configuration lock.")?
        .is_file()
    {
        return Err("Cursor configuration lock must be a regular file.");
    }
    // Probe once so callers can observe a real contention event in tests.
    // SAFETY: file is open and remains live throughout the lock operation.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(file);
    }
    let error = io::Error::last_os_error();
    if error.kind() != io::ErrorKind::WouldBlock {
        return Err("Cannot lock Cursor configuration.");
    }
    on_contention();
    loop {
        // SAFETY: the file is open, and its descriptor stays live through flock.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
            return Ok(file);
        }
        if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return Err("Cannot lock Cursor configuration.");
        }
    }
}

/// The lock pathname and content must still refer to the record we read.
pub(super) struct Snapshot {
    pub(super) metadata: fs::Metadata,
    pub(super) bytes: Vec<u8>,
}

impl Snapshot {
    pub(super) fn read(file: &mut File, dir: &Path) -> Result<Self, &'static str> {
        let metadata = file.metadata().map_err(|_| super::AMBIGUOUS)?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| super::AMBIGUOUS)?;
        let mut bytes = Vec::new();
        file.take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| super::AMBIGUOUS)?;
        if bytes.len() > 1024 * 1024 {
            return Err(super::AMBIGUOUS);
        }
        let snapshot = Self { metadata, bytes };
        snapshot.check(file, dir)?;
        Ok(snapshot)
    }

    pub(super) fn check(&self, file: &mut File, dir: &Path) -> Result<(), &'static str> {
        let path = dir.join(".hooks.json.arc.lock");
        let mut current = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| super::AMBIGUOUS)?;
        let held = file.metadata().map_err(|_| super::AMBIGUOUS)?;
        let metadata = current.metadata().map_err(|_| super::AMBIGUOUS)?;
        let identity = |m: &fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.mode(),
                m.uid(),
                m.gid(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        };
        if identity(&self.metadata) != identity(&held) || identity(&held) != identity(&metadata) {
            return Err(super::AMBIGUOUS);
        }
        let mut bytes = Vec::new();
        (&mut current)
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| super::AMBIGUOUS)?;
        let after = current.metadata().map_err(|_| super::AMBIGUOUS)?;
        let held_after = file.metadata().map_err(|_| super::AMBIGUOUS)?;
        let path_after =
            fs::symlink_metadata(dir.join(".hooks.json.arc.lock")).map_err(|_| super::AMBIGUOUS)?;
        if bytes != self.bytes
            || !path_after.is_file()
            || identity(&self.metadata) != identity(&after)
            || identity(&after) != identity(&held_after)
            || identity(&after) != identity(&path_after)
        {
            return Err(super::AMBIGUOUS);
        }
        Ok(())
    }

    // Parse the exact bytes whose identity and contents will be checked before
    // publication or record changes, not a separate read of the lock file.
    pub(super) fn owner(&self) -> Result<Option<serde_json::Value>, &'static str> {
        parse_owner(&self.bytes)
    }
}

pub(super) fn parse_owner(bytes: &[u8]) -> Result<Option<serde_json::Value>, &'static str> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let entry = serde_json::from_slice(bytes).map_err(|_| super::AMBIGUOUS)?;
    if !super::owned_entry(&entry) {
        return Err(super::AMBIGUOUS);
    }
    Ok(Some(entry))
}

const RECORD_ERROR: &str =
    "Cannot record Cursor hook ownership; retry after reviewing hooks.json and the ARC lock file.";

pub(super) fn owner_bytes(entry: Option<&serde_json::Value>) -> Result<Vec<u8>, &'static str> {
    entry
        .map(serde_json::to_vec)
        .transpose()
        .map_err(|_| RECORD_ERROR)?
        .map_or_else(|| Ok(Vec::new()), Ok)
}

pub(super) fn write_owner(file: &mut File, bytes: &[u8]) -> Result<(), &'static str> {
    file.seek(SeekFrom::Start(0)).map_err(|_| RECORD_ERROR)?;
    file.set_len(0).map_err(|_| RECORD_ERROR)?;
    file.write_all(bytes).map_err(|_| RECORD_ERROR)?;
    file.sync_data().map_err(|_| RECORD_ERROR)
}
