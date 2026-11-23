//! The scan-interval store of native process attribution.
//!
//! One aggregate row per interval `[start_ms, end_ms)`, identified by its
//! scanner and the sequence number of the scan that closed it (unique):
//! `measured`, with CPU core-seconds per confidence and vendor, or `failed`
//! for a stretch the scanner could not observe. A measured interval's bounds
//! are a conservative envelope of both native reads of its deltas: the start
//! of the earlier scan and the end of the later one, rounded outward. So
//! consecutive envelopes overlap in wall time while their deltas are
//! distinct; coverage is their union, never their sum.
//!
//! Each scanner keeps one durable **frontier** row: the scan whose reads are
//! the open baseline of its next interval, with its wall and boot-clock
//! readings, committed in the same transaction as every interval. A
//! frontier that a later interval did not close is an unresolved stretch
//! from its **floor** (`low_ms`, at most its start): marked failed by its
//! scanner when it can, or left by a scanner that stopped or crashed. A wall
//! clock that stepped back lowers the floor to what both clocks imply
//! ([`floor_of`]). The next baseline of any scanner turns every frontier into
//! one failed interval from the lowest floor to that baseline's end.
//!
//! Every frontier is also a **fence**: until the scanner that wrote it
//! certifies, on a clean stop, that
//! it holds nothing unrecorded ([`IntervalStore::seal`]), the frontier of a
//! scanner that is not running is unresolved from wall time 0. A stopped
//! scanner may have read a backward wall-clock step it could not record,
//! and a corrected clock no longer shows it; so no window before an
//! unsealed frontier regains a complete reading, and the next baseline's
//! failed interval starts at 0. A seal never depends on a write during an
//! outage: when it cannot be written, the fence stays.
//! Before the first scan, an independent lifecycle witness is durably armed.
//! It remains unclean until the frontier and any failure are safely sealed,
//! including when the first baseline cannot be committed at all.
//!
//! Only aggregates: no PID, argv, path, cwd or process name of any process;
//! a scanner is a random per-daemon-run id. The store is a lane-private
//! SQLite file beside the main database (`location.rs`), owned by the
//! database's owner and closed to group and other, versioned by its own
//! `user_version` (`schema.rs`). Writes are one immediate transaction;
//! readers open it read-only. Rows are bounded by age ([`RETENTION_MS`]) and
//! count ([`MAX_INTERVALS`]).
use crate::jobs::{JobError, JobResult};
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};
use std::{
    fs,
    io::{ErrorKind, Read},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

#[path = "intervals/clock.rs"]
mod clock;
#[path = "intervals/lifecycle.rs"]
mod lifecycle;
#[path = "intervals/location.rs"]
mod location;
#[path = "intervals/merge.rs"]
mod merge;
#[path = "intervals/schema.rs"]
mod schema;
#[path = "intervals/select.rs"]
mod select;
#[path = "intervals/write.rs"]
mod write;

/// Intervals ending before `now - RETENTION_MS` are deleted by each write.
pub const RETENTION_MS: u64 = 35 * 24 * 60 * 60 * 1000;
/// At most this many intervals are kept; the oldest are deleted first.
pub const MAX_INTERVALS: i64 = 200_000;
pub use clock::{floor_of, BootClock, Frontier, ScanPoint};
pub use lifecycle::LifecycleGuard;
use location::{check, writable};

pub(super) const MAX_MS: u64 = 9_007_199_254_740_991;

#[cfg(test)]
thread_local! {
    /// Test seam: a merge that committed leaves the earlier directory in
    /// place, as an interrupted cleanup would.
    pub(super) static KEEP_EARLIER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The aggregates of one measured interval, in core-seconds; its bounds are
/// the envelope of both reads of its deltas.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Interval {
    pub start_ms: u64,
    pub end_ms: u64,
    pub proven: Vec<(String, f64)>,
    pub estimated: Vec<(String, f64)>,
    pub unattributed: f64,
    /// Processes of the closing scan that contributed nothing.
    pub skipped: u64,
}

/// One daemon run's scanner: 128 random bits, lowercase hex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannerId(String);
impl ScannerId {
    /// `None` when no randomness can be read; the scanner then records nothing.
    pub fn random() -> Option<Self> {
        let mut bytes = [0u8; 16];
        fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut bytes))
            .ok()?;
        Some(Self(bytes.iter().map(|b| format!("{b:02x}")).collect()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The interval store beside one main database file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntervalStore {
    file: PathBuf,
    earlier: PathBuf,
    database: PathBuf,
}

/// An unresolved frontier whose floor lies before a window's end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Open {
    pub scanner: String,
    pub origin: String,
    pub seq: u64,
    pub start_ms: u64,
    pub low_ms: u64,
    pub failed: bool,
    /// Its stopped scanner certified it: its floor is `low_ms`, not 0.
    pub sealed: bool,
    pub boot: Option<BootClock>,
    /// Divergent frontier histories share this scanner identity.
    pub conflicted: bool,
}
impl Open {
    /// The floor of its unresolved stretch for a reader whose running
    /// scanner is `live`: an unsealed fence of another scanner reaches 0.
    pub fn floor(&self, live: Option<&str>) -> u64 {
        if self.sealed || (!self.conflicted && live == Some(self.scanner.as_str())) {
            self.low_ms
        } else {
            0
        }
    }
}

/// The selected part of a window, read in one store snapshot.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct Selection {
    pub intervals: u64,
    pub first_start_ms: Option<u64>,
    pub last_end_ms: Option<u64>,
    /// The length of the union of the included envelopes.
    pub covered_ms: u64,
    pub skipped: u64,
    /// Failed intervals overlapping the window.
    pub failed: u64,
    pub open: Vec<Open>,
    /// Version-1 bounds may touch the window.
    pub uncertain: bool,
    pub groups: Vec<(String, String, f64)>,
}

fn sql(value: u64) -> i64 {
    value.min(MAX_MS) as i64
}

impl IntervalStore {
    /// The store of `database`, with its directory resolved so the daemon's
    /// configured path and a connection's reported path name one store.
    pub fn beside(database: &Path) -> Option<Self> {
        let name = database.file_name()?.to_str()?;
        let parent = database
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = fs::canonicalize(parent).ok()?;
        let (current, earlier) = location::directories(&parent, name);
        Some(Self {
            file: current.join(location::FILE),
            earlier,
            database: parent.join(name),
        })
    }

    pub fn path(&self) -> &Path {
        &self.file
    }

    /// The earlier builds' location of this store's directory.
    pub fn earlier_directory(&self) -> &Path {
        &self.earlier
    }

    /// [`floor_of`] over the durable frontiers, read-only. Errors when the
    /// store exists but cannot be read.
    pub fn reversal_floor(&self, at: &ScanPoint) -> JobResult<Option<u64>> {
        let mut frontiers = Vec::new();
        for connection in self.open_for_read()? {
            frontiers.extend(select::frontiers(&connection)?);
        }
        Ok(floor_of(&frontiers, at))
    }

    /// One immediate transaction: relocation, schema, `write` (given the
    /// durable frontiers), then retention when `now_ms` is set.
    fn transaction(
        &self,
        now_ms: u64,
        write: impl FnOnce(&Transaction<'_>, &[Frontier]) -> JobResult<()>,
    ) -> JobResult<()> {
        let (mut connection, earlier) = self.open_for_write()?;
        if let location::Earlier::Merge(file) = &earlier {
            location::rollback_journal(&connection)?;
            // Bring the earlier file to this version before copying it.
            let mut old = writable(file)?;
            location::rollback_journal(&old)?;
            let tx = old.transaction_with_behavior(TransactionBehavior::Immediate)?;
            schema::upgrade(&tx, now_ms)?;
            tx.commit()?;
            drop(old);
            connection.execute(
                "ATTACH DATABASE ?1 AS legacy",
                [file.to_str().ok_or(JobError::StorageFailure)?],
            )?;
            let mode: String =
                connection.query_row("PRAGMA legacy.journal_mode", [], |r| r.get(0))?;
            if !mode.eq_ignore_ascii_case("delete") {
                return Err(JobError::StorageFailure);
            }
            connection.execute_batch("PRAGMA legacy.synchronous=FULL")?;
        }
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        schema::upgrade(&tx, now_ms)?;
        if matches!(earlier, location::Earlier::Merge(_)) {
            merge::attached(&tx)?;
        }
        let frontiers = select::frontiers(&tx)?;
        write(&tx, &frontiers)?;
        if now_ms > 0 {
            let cutoff = sql(now_ms.saturating_sub(RETENTION_MS));
            tx.execute(
                "DELETE FROM attribution_intervals WHERE end_ms<?1",
                [cutoff],
            )?;
            // By end, not id: a merged row gets a new id.
            tx.execute(
                "DELETE FROM attribution_intervals WHERE id IN (SELECT id FROM attribution_intervals
                   ORDER BY end_ms DESC, id DESC LIMIT -1 OFFSET ?1)",
                [MAX_INTERVALS],
            )?;
        }
        tx.commit()?;
        if matches!(earlier, location::Earlier::Merge(_)) {
            connection.execute("DETACH DATABASE legacy", [])?;
            #[cfg(test)]
            if KEEP_EARLIER.get() {
                return Ok(());
            }
            location::remove(&self.earlier);
        }
        Ok(())
    }

    fn owner(&self) -> JobResult<u32> {
        Ok(fs::symlink_metadata(&self.database)
            .map_err(|_| JobError::StorageFailure)?
            .uid())
    }

    /// The directory and file are created owner-only, beside and owned like
    /// the database; anything else (a symlink, another owner, an open mode)
    /// is refused rather than written.
    fn open_for_write(&self) -> JobResult<(Connection, location::Earlier)> {
        let owner = self.owner()?;
        let directory = self.file.parent().ok_or(JobError::StorageFailure)?;
        let earlier = location::relocate(directory, &self.earlier, owner)?;
        match fs::DirBuilder::new().mode(0o700).create(directory) {
            Ok(()) => (),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => (),
            Err(_) => return Err(JobError::StorageFailure),
        }
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&self.file)
        {
            Ok(_) => (),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => (),
            Err(_) => return Err(JobError::StorageFailure),
        }
        check(directory, owner, true)?;
        check(&self.file, owner, false)?;
        Ok((writable(&self.file)?, earlier))
    }

    /// Every store file that exists, read-only; none when nothing was ever
    /// written.
    fn open_for_read(&self) -> JobResult<Vec<Connection>> {
        let directory = self.file.parent().ok_or(JobError::StorageFailure)?;
        let files = location::readable(directory, &self.earlier)?;
        if files.is_empty() {
            return Ok(Vec::new());
        }
        let owner = self.owner()?;
        let mut connections = Vec::new();
        for file in files {
            check(file.parent().ok_or(JobError::StorageFailure)?, owner, true)?;
            check(&file, owner, false)?;
            let connection = Connection::open_with_flags(
                &file,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            connection.busy_timeout(Duration::from_secs(1))?;
            connections.push(connection);
        }
        Ok(connections)
    }

    /// Measured intervals wholly inside `[from_ms, to_ms)`, the failed
    /// intervals overlapping it, all frontiers (so conflicting identities
    /// can be detected), and any uncertain range overlapping it, in one read snapshot
    /// of each store file (the earlier location first).
    pub(super) fn select(&self, from_ms: u64, to_ms: u64) -> JobResult<Selection> {
        let mut raws = Vec::new();
        for mut connection in self.open_for_read()? {
            raws.push(select::read(&mut connection, from_ms, to_ms)?);
        }
        Ok(select::combine(raws))
    }
}
