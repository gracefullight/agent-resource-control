//! A durable admission witness, independent of the interval database.
//!
//! A scanner must commit its unclean witness before it attempts any scan.
//! It clears the witness only after sealing all of its interval failures.
//! Thus an unwritable interval store cannot erase the fact that the scanner
//! ran; a later scanner treats an unclean predecessor as uncertain from 0.
//! The file contains only a random scanner id and a clean/unclean bit.
use super::{location, IntervalStore, ScannerId};
use crate::jobs::{JobError, JobResult};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::{
    fs,
    io::ErrorKind,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::Duration,
};

/// Exclusive for one scanner lifetime: another daemon sharing this store
/// cannot overwrite its witness while it may still observe a failure.
pub struct LifecycleGuard {
    _lock: fs::File,
    path: PathBuf,
}

impl IntervalStore {
    fn witness_path(&self) -> JobResult<PathBuf> {
        let name = self
            .database
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(JobError::StorageFailure)?;
        Ok(self
            .database
            .parent()
            .ok_or(JobError::StorageFailure)?
            .join(format!(".{name}.attribution-lifecycle.sqlite3")))
    }

    fn lock_path(&self) -> JobResult<PathBuf> {
        let mut file = self.witness_path()?.into_os_string();
        file.push(".lock");
        Ok(file.into())
    }

    /// `durable` syncs a newly created file and its directory entry; the
    /// lifetime lock needs none, since a lock is never read back after a crash.
    fn private_file(&self, file: &Path, create: bool, durable: bool) -> JobResult<()> {
        let owner = self.owner()?;
        if create {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(file)
            {
                Ok(_) if !durable => (),
                Ok(created) => {
                    created.sync_all().map_err(|_| JobError::StorageFailure)?;
                    // A committed SQLite file is not a durable witness if its
                    // new directory entry can disappear after a crash.
                    fs::File::open(file.parent().ok_or(JobError::StorageFailure)?)
                        .and_then(|dir| dir.sync_all())
                        .map_err(|_| JobError::StorageFailure)?;
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => (),
                Err(_) => return Err(JobError::StorageFailure),
            }
        }
        location::check(file, owner, false)
    }

    /// Arming commits the unclean witness with a full device flush: that is
    /// the write a later reader must not lose. Sealing does not: a lost seal
    /// leaves the witness unclean, which only makes later readers more
    /// conservative, so it is committed like the interval store itself
    /// (`synchronous=FULL` on a rollback journal) and a daemon's stop does
    /// not wait for a drive-cache flush.
    fn open_witness(&self, arming: bool) -> JobResult<Connection> {
        let file = self.witness_path()?;
        self.private_file(&file, arming, true)?;
        let connection = location::writable(&file)?;
        location::rollback_journal(&connection)?;
        if arming {
            connection.execute_batch("PRAGMA fullfsync=ON")?;
        }
        Ok(connection)
    }

    /// Commit an unclean witness before a scanner is allowed to read native
    /// processes. Returns true when earlier history cannot be trusted. A
    /// missing witness beside an existing interval store is deliberately
    /// uncertain: the missing evidence cannot certify an older clean run.
    pub fn arm_lifecycle(&self, scanner: &ScannerId) -> JobResult<(LifecycleGuard, bool)> {
        let old_store =
            self.path().exists() || self.earlier_directory().join(location::FILE).exists();
        // Keep the lifetime lock on a different inode from SQLite's own
        // transaction locks. Both files are private to the database owner.
        let path = self.lock_path()?;
        self.private_file(&path, true, false)?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|_| JobError::StorageFailure)?;
        lock.try_lock().map_err(|_| JobError::Busy)?;
        let guard = LifecycleGuard { _lock: lock, path };
        let mut connection = self.open_witness(true)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS scanner_lifecycle (
            id INTEGER PRIMARY KEY CHECK(id=1), scanner TEXT NOT NULL
              CHECK(length(scanner)=32 AND scanner NOT GLOB '*[^0-9a-f]*'),
            clean INTEGER NOT NULL CHECK(clean IN (0,1)))",
        )?;
        let prior: Option<i64> = tx
            .query_row("SELECT clean FROM scanner_lifecycle WHERE id=1", [], |r| {
                r.get(0)
            })
            .optional()?;
        let uncertain = prior.map_or(old_store, |clean| clean != 1);
        tx.execute(
            "INSERT INTO scanner_lifecycle(id,scanner,clean) VALUES(1,?1,0)
             ON CONFLICT(id) DO UPDATE SET scanner=excluded.scanner,clean=0",
            [scanner.as_str()],
        )?;
        tx.commit()?;
        Ok((guard, uncertain))
    }

    /// Certify a scanner only after its interval frontier and every pending
    /// failure were committed. Another scanner's witness must never be
    /// cleared by this one.
    pub fn seal_lifecycle(&self, guard: &LifecycleGuard, scanner: &ScannerId) -> JobResult<()> {
        if guard.path != self.lock_path()? {
            return Err(JobError::Conflict);
        }
        let mut connection = self.open_witness(false)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let count = tx.execute(
            "UPDATE scanner_lifecycle SET clean=1 WHERE id=1 AND scanner=?1 AND clean=0",
            params![scanner.as_str()],
        )?;
        if count != 1 {
            return Err(JobError::Conflict);
        }
        tx.commit()?;
        Ok(())
    }

    /// An abandoned witness remains uncertain even for an offline reader.
    /// A running scanner handles its own prior uncertainty in `LiveScanner`.
    pub(crate) fn lifecycle_pending(&self, live: Option<&str>) -> JobResult<bool> {
        let file = self.witness_path()?;
        match fs::symlink_metadata(&file) {
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(_) => return Err(JobError::StorageFailure),
            Ok(_) => (),
        }
        location::check(&file, self.owner()?, false)?;
        let connection = Connection::open_with_flags(
            &file,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.busy_timeout(Duration::from_secs(1))?;
        let has_table: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='scanner_lifecycle')",
            [], |r| r.get(0),
        )?;
        if !has_table {
            return Ok(true);
        }
        let row: Option<(String, i64)> = connection
            .query_row(
                "SELECT scanner,clean FROM scanner_lifecycle WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(row.is_none_or(|(scanner, clean)| clean != 1 && live != Some(scanner.as_str())))
    }
}
