//! Detect stale settings snapshots before publishing a replacement.
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

const CHANGED: &str = "Cursor configuration changed during the update; retry the operation.";

pub(super) struct Snapshot {
    pub source: File,
    pub bytes: Vec<u8>,
    metadata: Metadata,
}

fn identity_matches(left: &Metadata, right: &Metadata) -> bool {
    (
        left.dev(),
        left.ino(),
        left.len(),
        left.mode(),
        left.uid(),
        left.gid(),
        left.mtime(),
        left.mtime_nsec(),
        left.ctime(),
        left.ctime_nsec(),
    ) == (
        right.dev(),
        right.ino(),
        right.len(),
        right.mode(),
        right.uid(),
        right.gid(),
        right.mtime(),
        right.mtime_nsec(),
        right.ctime(),
        right.ctime_nsec(),
    )
}

fn open(path: &Path) -> Result<Option<File>, &'static str> {
    match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file)
            if file
                .metadata()
                .map_err(|_| "Cannot inspect Cursor hooks.json.")?
                .is_file() =>
        {
            Ok(Some(file))
        }
        Ok(_) => Err("Cursor hooks.json must be a regular file."),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("Cannot open Cursor hooks.json."),
    }
}

pub(super) fn read(path: &Path) -> Result<Option<Snapshot>, &'static str> {
    let Some(mut source) = open(path)? else {
        return Ok(None);
    };
    let metadata = source
        .metadata()
        .map_err(|_| "Cannot inspect Cursor hooks.json.")?;
    let mut bytes = Vec::new();
    source
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read Cursor hooks.json.")?;
    let snapshot = Snapshot {
        source,
        bytes,
        metadata,
    };
    check(path, Some(&snapshot))?;
    Ok(Some(snapshot))
}

pub(super) fn check(path: &Path, snapshot: Option<&Snapshot>) -> Result<(), &'static str> {
    let Some(snapshot) = snapshot else {
        return match fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(CHANGED),
        };
    };
    let mut current = open(path).map_err(|_| CHANGED)?.ok_or(CHANGED)?;
    let before = current.metadata().map_err(|_| CHANGED)?;
    let path_metadata = fs::symlink_metadata(path).map_err(|_| CHANGED)?;
    if !path_metadata.file_type().is_file()
        || !identity_matches(&snapshot.metadata, &before)
        || !identity_matches(&before, &path_metadata)
    {
        return Err(CHANGED);
    }
    let mut bytes = Vec::new();
    current.read_to_end(&mut bytes).map_err(|_| CHANGED)?;
    let after = current.metadata().map_err(|_| CHANGED)?;
    let path_after = fs::symlink_metadata(path).map_err(|_| CHANGED)?;
    if bytes != snapshot.bytes
        || !identity_matches(&before, &after)
        || !identity_matches(&after, &path_after)
    {
        return Err(CHANGED);
    }
    Ok(())
}
