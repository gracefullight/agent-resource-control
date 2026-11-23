//! Where the interval store lives, and the move from its earlier location.
//!
//! The store is `<parent>/.<db>.attribution/intervals.sqlite3`: a hidden
//! sibling of the main database whose name does not begin with the
//! database's file name, so nothing that collects the database and its
//! journal files by that prefix ever meets a directory. Earlier builds used
//! `<parent>/<db>.attribution/`. A writer moves that directory with one
//! `rename` (atomic: an interrupted move leaves either location whole). When
//! both exist, the writer switches both files to rollback journals and
//! copies the earlier file into the current one in one transaction, and
//! removes the earlier directory after the commit; a removal that did not
//! happen is repeated by the next write, which finds nothing left to copy,
//! so no row is lost or counted twice. Readers read both while both exist.
use crate::jobs::{JobError, JobResult};
use rusqlite::{Connection, OpenFlags};
use std::{
    fs,
    io::ErrorKind,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) const FILE: &str = "intervals.sqlite3";

/// `(current, earlier)` store directories of the database `name` in `parent`.
pub(super) fn directories(parent: &Path, name: &str) -> (PathBuf, PathBuf) {
    (
        parent.join(format!(".{name}.attribution")),
        parent.join(format!("{name}.attribution")),
    )
}

/// What a writer must still do with the earlier location.
pub(super) enum Earlier {
    None,
    /// Copy this file into the current store, then remove its directory.
    Merge(PathBuf),
}

fn exists(path: &Path) -> JobResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err(JobError::StorageFailure),
    }
}

/// Move the earlier directory into place when the current one is absent.
/// An earlier entry that is not the owner's private directory is refused.
pub(super) fn relocate(current: &Path, earlier: &Path, owner: u32) -> JobResult<Earlier> {
    if !exists(earlier)? {
        return Ok(Earlier::None);
    }
    check(earlier, owner, true)?;
    if !exists(current)? {
        return match fs::rename(earlier, current) {
            Ok(()) => Ok(Earlier::None),
            // Another writer moved it first.
            Err(_) if exists(current)? && !exists(earlier)? => Ok(Earlier::None),
            Err(_) => Err(JobError::StorageFailure),
        };
    }
    let file = earlier.join(FILE);
    if exists(&file)? {
        check(&file, owner, false)?;
        Ok(Earlier::Merge(file))
    } else {
        remove(earlier);
        Ok(Earlier::None)
    }
}

/// Remove a merged earlier directory with the file's journal files. A
/// failure leaves it for the next write, which merges it again.
pub(super) fn remove(earlier: &Path) {
    for suffix in ["", "-journal", "-wal", "-shm"] {
        let _ = fs::remove_file(earlier.join(format!("{FILE}{suffix}")));
    }
    let _ = fs::remove_dir(earlier);
}

/// The store files a reader reads: while it still exists the earlier one,
/// then the current file. In this order a merge that commits between the
/// two reads is seen whole in the current file: a merge empties the earlier
/// file in the same atomic transaction that fills the current one.
pub(super) fn readable(current: &Path, earlier: &Path) -> JobResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    for directory in [earlier, current] {
        let file = directory.join(FILE);
        if exists(&file)? {
            files.push(file);
        }
    }
    Ok(files)
}

pub(super) fn writable(file: &Path) -> JobResult<Connection> {
    let connection = Connection::open_with_flags(
        file,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(Duration::from_secs(1))?;
    connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;")?;
    Ok(connection)
}

/// SQLite's multi-file commit is crash-atomic only with disk-backed main
/// and rollback journals, not WAL. Refuse a merge if either connection
/// cannot leave WAL (for example, while another connection holds it).
pub(super) fn rollback_journal(connection: &Connection) -> JobResult<()> {
    let mode: String = connection.query_row("PRAGMA journal_mode=DELETE", [], |r| r.get(0))?;
    if !mode.eq_ignore_ascii_case("delete") {
        return Err(JobError::StorageFailure);
    }
    Ok(())
}

pub(super) fn check(path: &Path, owner: u32, directory: bool) -> JobResult<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| JobError::StorageFailure)?;
    let kind = if directory {
        metadata.is_dir()
    } else {
        metadata.is_file()
    };
    if !kind || metadata.uid() != owner || metadata.mode() & 0o077 != 0 {
        return Err(JobError::StorageFailure);
    }
    Ok(())
}
