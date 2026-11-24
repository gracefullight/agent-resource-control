//! The interval store's schema and its non-destructive upgrades.
//!
//! Version 4 is current. Every upgrade runs inside the writer's one
//! immediate transaction, so an interrupted upgrade leaves the earlier
//! version intact.
//! - **Version 1** recorded scan-completion bounds, which are not an
//!   envelope of a delta's reads. Its tables are kept, renamed
//!   `attribution_v1_*`, never included in a sum, and one
//!   `attribution_uncertain` range from 0 to the upgrade marks every window
//!   they could touch explicitly uncertain. Its frontiers stay unresolved.
//! - **Version 2** stored envelopes; only its frontier gains the lowest wall
//!   time of its unresolved stretch and the boot-clock reading of its scan.
//! - **Version 3** capped boot-clock readings at 2^53 - 1 ns (about 104.25
//!   days of uptime). Its frontiers gain `sealed` (0: no stopped scanner
//!   certified them), and a reading at the cap is dropped: it cannot be
//!   compared, so it never implies a step or its absence.
use crate::jobs::{JobError, JobResult};
use rusqlite::{params, Transaction};

pub(super) const SCHEMA_VERSION: i64 = 5;
/// The cap version 3 applied to boot-clock nanoseconds.
pub(super) const CAPPED_BOOT_NS: i64 = 9_007_199_254_740_991;
/// The reason stored with an uncertain range of version-1 bounds.
pub(super) const LEGACY_BOUNDS: &str = "legacy_bounds";
const SCANNER_CHECK: &str = "length(scanner)=32 AND scanner NOT GLOB '*[^0-9a-f]*'";
const INTERVALS: &str = "
CREATE TABLE attribution_intervals (
  id INTEGER PRIMARY KEY,
  scanner TEXT NOT NULL CHECK(SCANNER_CHECK),
  seq INTEGER NOT NULL CHECK(typeof(seq)='integer' AND seq >= 1),
  start_ms INTEGER NOT NULL
    CHECK(typeof(start_ms)='integer' AND start_ms BETWEEN 0 AND 9007199254740991),
  end_ms INTEGER NOT NULL
    CHECK(typeof(end_ms)='integer' AND end_ms > start_ms AND end_ms <= 9007199254740991),
  status TEXT NOT NULL CHECK(status IN ('measured','failed')),
  skipped_processes INTEGER NOT NULL DEFAULT 0
    CHECK(skipped_processes >= 0 AND (status='measured' OR skipped_processes=0)),
  UNIQUE(scanner, seq)
);
CREATE INDEX idx_attribution_intervals_end ON attribution_intervals(end_ms);
CREATE INDEX idx_attribution_intervals_start ON attribution_intervals(start_ms);
CREATE TABLE attribution_interval_cpu (
  interval_id INTEGER NOT NULL REFERENCES attribution_intervals(id) ON DELETE CASCADE,
  confidence TEXT NOT NULL CHECK(confidence IN ('proven','estimated','unattributed')),
  vendor TEXT NOT NULL CHECK(
    (confidence='unattributed' AND vendor='') OR
    (confidence='proven' AND vendor IN ('claude-code','cursor','codex','none')) OR
    (confidence='estimated' AND vendor IN ('claude-code','cursor','codex'))),
  cpu_core_seconds REAL NOT NULL CHECK(cpu_core_seconds >= 0),
  PRIMARY KEY(interval_id, confidence, vendor)
) WITHOUT ROWID;
";
const FRONTIER: &str = "
CREATE TABLE attribution_frontier (
  scanner TEXT PRIMARY KEY NOT NULL CHECK(SCANNER_CHECK),
  seq INTEGER NOT NULL CHECK(typeof(seq)='integer' AND seq >= 1),
  start_ms INTEGER NOT NULL
    CHECK(typeof(start_ms)='integer' AND start_ms BETWEEN 0 AND 9007199254740991),
  failed INTEGER NOT NULL DEFAULT 0 CHECK(failed IN (0,1)),
  low_ms INTEGER NOT NULL CHECK(typeof(low_ms)='integer' AND low_ms BETWEEN 0 AND start_ms),
  boot_id TEXT CHECK(boot_id IS NULL OR length(boot_id) BETWEEN 1 AND 64),
  boot_ns INTEGER CHECK(boot_ns IS NULL OR (typeof(boot_ns)='integer' AND boot_ns >= 0)),
  sealed INTEGER NOT NULL DEFAULT 0 CHECK(sealed IN (0,1)),
  CHECK((boot_id IS NULL) = (boot_ns IS NULL))
) WITHOUT ROWID;
";
const UNCERTAIN: &str = "
CREATE TABLE attribution_uncertain (
  id INTEGER PRIMARY KEY,
  from_ms INTEGER NOT NULL CHECK(typeof(from_ms)='integer' AND from_ms >= 0),
  to_ms INTEGER NOT NULL
    CHECK(typeof(to_ms)='integer' AND to_ms > from_ms AND to_ms <= 9007199254740991),
  reason TEXT NOT NULL CHECK(reason IN ('legacy_bounds'))
);
";
const V1_TABLES: [&str; 3] = [
    "attribution_intervals",
    "attribution_interval_cpu",
    "attribution_frontier",
];

fn ddl(text: &str) -> String {
    text.replace("SCANNER_CHECK", SCANNER_CHECK)
}

pub(super) fn table_exists(tx: &Transaction<'_>, schema: &str, name: &str) -> JobResult<bool> {
    Ok(tx.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM {schema}.sqlite_master WHERE type='table' AND name=?1)"
        ),
        [name],
        |r| r.get(0),
    )?)
}

/// Bring `main` of `tx` to [`SCHEMA_VERSION`]; `now_ms` bounds the
/// uncertain range of version-1 data.
pub(super) fn upgrade(tx: &Transaction<'_>, now_ms: u64) -> JobResult<()> {
    let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    match version {
        SCHEMA_VERSION => return Ok(()),
        0 => tx.execute_batch(&ddl(&[INTERVALS, FRONTIER, UNCERTAIN].concat()))?,
        1 => preserve_v1(tx, now_ms)?,
        2 => {
            tx.execute_batch(&ddl(&format!(
                "ALTER TABLE attribution_frontier RENAME TO attribution_frontier_v2;
                 {FRONTIER}
                 INSERT INTO attribution_frontier(scanner,seq,start_ms,failed,low_ms)
                   SELECT scanner,seq,start_ms,failed,start_ms FROM attribution_frontier_v2;
                 DROP TABLE attribution_frontier_v2;
                 {UNCERTAIN}"
            )))?;
        }
        4 => (),
        3 => tx.execute_batch(&format!(
            "ALTER TABLE attribution_frontier
               ADD COLUMN sealed INTEGER NOT NULL DEFAULT 0 CHECK(sealed IN (0,1));
             UPDATE attribution_frontier SET boot_id=NULL,boot_ns=NULL
               WHERE boot_ns>={CAPPED_BOOT_NS};"
        ))?,
        _ => return Err(JobError::StorageFailure),
    }
    // SQLite cannot alter a CHECK constraint. Rebuild only the private CPU
    // child table inside this transaction, copying every existing row.
    if (2..=4).contains(&version) {
        tx.execute_batch(
            "ALTER TABLE attribution_interval_cpu RENAME TO attribution_interval_cpu_v4;
            CREATE TABLE attribution_interval_cpu (
              interval_id INTEGER NOT NULL REFERENCES attribution_intervals(id) ON DELETE CASCADE,
              confidence TEXT NOT NULL CHECK(confidence IN ('proven','estimated','unattributed')),
              vendor TEXT NOT NULL CHECK(
                (confidence='unattributed' AND vendor='') OR
                (confidence='proven' AND vendor IN ('claude-code','cursor','codex','none')) OR
                (confidence='estimated' AND vendor IN ('claude-code','cursor','codex'))),
              cpu_core_seconds REAL NOT NULL CHECK(cpu_core_seconds >= 0),
              PRIMARY KEY(interval_id,confidence,vendor)) WITHOUT ROWID;
            INSERT INTO attribution_interval_cpu SELECT * FROM attribution_interval_cpu_v4;
            DROP TABLE attribution_interval_cpu_v4;",
        )?;
    }
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

/// Keep every version-1 table under an `attribution_v1_` name, create the
/// current tables, keep its frontiers unresolved and mark `[0, upgrade]`
/// uncertain when it held any row.
fn preserve_v1(tx: &Transaction<'_>, now_ms: u64) -> JobResult<()> {
    let mut kept = Vec::new();
    for table in V1_TABLES {
        if table_exists(tx, "main", table)? {
            tx.execute_batch(&format!(
                "ALTER TABLE {table} RENAME TO attribution_v1_{}",
                table.trim_start_matches("attribution_")
            ))?;
            kept.push(table);
        }
    }
    tx.execute_batch(
        "DROP INDEX IF EXISTS idx_attribution_intervals_end;
         DROP INDEX IF EXISTS idx_attribution_intervals_start;",
    )?;
    tx.execute_batch(&ddl(&[INTERVALS, FRONTIER, UNCERTAIN].concat()))?;
    let mut reach = now_ms;
    let mut rows = 0i64;
    if kept.contains(&"attribution_intervals") {
        let (n, end): (i64, Option<i64>) = tx.query_row(
            "SELECT count(*),MAX(end_ms) FROM attribution_v1_intervals",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        rows += n;
        reach = reach.max(end.unwrap_or(0).max(0) as u64);
    }
    if kept.contains(&"attribution_frontier") {
        let (n, start): (i64, Option<i64>) = tx.query_row(
            "SELECT count(*),MAX(start_ms) FROM attribution_v1_frontier",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        rows += n;
        reach = reach.max(start.unwrap_or(0).max(0) as u64 + 1);
        tx.execute(
            "INSERT INTO attribution_frontier(scanner,seq,start_ms,failed,low_ms)
             SELECT scanner,seq,start_ms,failed,start_ms FROM attribution_v1_frontier",
            [],
        )?;
    }
    if rows > 0 {
        tx.execute(
            "INSERT INTO attribution_uncertain(from_ms,to_ms,reason) VALUES(0,?1,?2)",
            params![reach.clamp(1, super::MAX_MS) as i64, LEGACY_BOUNDS],
        )?;
    }
    Ok(())
}
