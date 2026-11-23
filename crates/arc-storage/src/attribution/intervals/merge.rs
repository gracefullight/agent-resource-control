//! Merge both sidecars without discarding histories that reused an identity.
//!
//! A scanner is normally random and unique, but older copies can diverge.
//! An alias records the original scanner for every conflicting imported
//! history. Exact duplicate intervals (including their CPU children) are
//! reused; different rows get a new scanner identity and retain their own
//! children. The mapping also lets readers recognize a copy observed on
//! both sides of a commit. All copy and deletion happens in one transaction.
use super::{schema, ScannerId};
use crate::jobs::{JobError, JobResult};
use rusqlite::{params, OptionalExtension, Transaction};
use std::collections::BTreeSet;

const V1_INTERVALS: &str = "CREATE TABLE IF NOT EXISTS main.attribution_v1_intervals (
    id INTEGER PRIMARY KEY, scanner TEXT NOT NULL, seq INTEGER NOT NULL,
    start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL, status TEXT NOT NULL,
    skipped_processes INTEGER NOT NULL DEFAULT 0, UNIQUE(scanner,seq))";
const V1_CPU: &str = "CREATE TABLE IF NOT EXISTS main.attribution_v1_interval_cpu (
    interval_id INTEGER NOT NULL REFERENCES attribution_v1_intervals(id) ON DELETE CASCADE,
    confidence TEXT NOT NULL, vendor TEXT NOT NULL, cpu_core_seconds REAL NOT NULL,
    PRIMARY KEY(interval_id,confidence,vendor)) WITHOUT ROWID";
const V1_FRONTIER: &str = "CREATE TABLE IF NOT EXISTS main.attribution_v1_frontier (
    scanner TEXT PRIMARY KEY NOT NULL, seq INTEGER NOT NULL, start_ms INTEGER NOT NULL,
    failed INTEGER NOT NULL DEFAULT 0) WITHOUT ROWID";

fn scanners(tx: &Transaction<'_>, tables: &[&str]) -> JobResult<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    for table in tables {
        let mut statement = tx.prepare(&format!("SELECT DISTINCT scanner FROM legacy.{table}"))?;
        for scanner in statement.query_map([], |r| r.get(0))? {
            result.insert(scanner?);
        }
    }
    Ok(result)
}

fn occupied(tx: &Transaction<'_>, scanner: &str, tables: &[&str]) -> JobResult<bool> {
    for table in tables {
        let found: bool = tx.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM main.{table} WHERE scanner=?1)"),
            [scanner],
            |r| r.get(0),
        )?;
        if found {
            return Ok(true);
        }
    }
    Ok(tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM main.attribution_merge_scanners WHERE mapped=?1)",
        [scanner],
        |r| r.get(0),
    )?)
}

/// Build one fresh alias per conflicting legacy scanner. Random candidates
/// are checked against every existing identity before being committed.
fn aliases(tx: &Transaction<'_>, source: &[&str], target: &[&str]) -> JobResult<()> {
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS main.attribution_merge_scanners (
           mapped TEXT PRIMARY KEY NOT NULL, source TEXT NOT NULL) WITHOUT ROWID;
         CREATE TEMP TABLE attribution_import_scanners (
           source TEXT PRIMARY KEY NOT NULL, mapped TEXT NOT NULL,
           canonical TEXT NOT NULL) WITHOUT ROWID;",
    )?;
    let inherited = schema::table_exists(tx, "legacy", "attribution_merge_scanners")?;
    for scanner in scanners(tx, source)? {
        let canonical: String = if inherited {
            tx.query_row(
                "SELECT source FROM legacy.attribution_merge_scanners WHERE mapped=?1",
                [&scanner],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or_else(|| scanner.clone())
        } else {
            scanner.clone()
        };
        let mapped = if occupied(tx, &scanner, target)? {
            let alias = loop {
                let candidate = ScannerId::random().ok_or(JobError::StorageFailure)?;
                if !occupied(tx, candidate.as_str(), target)? {
                    break candidate.as_str().to_owned();
                }
            };
            alias
        } else {
            scanner.clone()
        };
        if mapped != canonical {
            tx.execute(
                "INSERT INTO main.attribution_merge_scanners(mapped,source) VALUES(?1,?2)",
                params![mapped, canonical],
            )?;
        }
        tx.execute(
            "INSERT INTO temp.attribution_import_scanners(source,mapped,canonical) VALUES(?1,?2,?3)",
            params![scanner, mapped, canonical],
        )?;
    }
    Ok(())
}

/// Copy parents before children. Reuse a row only if its entire content,
/// including the set of CPU children, equals the imported row.
fn intervals(tx: &Transaction<'_>, parent: &str, cpu: &str) -> JobResult<()> {
    let ids: Vec<i64> = tx
        .prepare(&format!("SELECT id FROM legacy.{parent} ORDER BY id"))?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for id in ids {
        let existing: Option<i64> = tx
            .query_row(
                &format!(
                    "SELECT m.id FROM main.{parent} m JOIN legacy.{parent} l ON l.id=?1
                 JOIN temp.attribution_import_scanners s ON s.source=l.scanner
                 WHERE m.seq=l.seq AND m.start_ms=l.start_ms AND m.end_ms=l.end_ms
                   AND m.status=l.status AND m.skipped_processes=l.skipped_processes
                   AND (m.scanner=s.canonical OR EXISTS(
                     SELECT 1 FROM main.attribution_merge_scanners a
                     WHERE a.source=s.canonical AND a.mapped=m.scanner))
                   AND NOT EXISTS(SELECT confidence,vendor,cpu_core_seconds FROM main.{cpu}
                     WHERE interval_id=m.id EXCEPT SELECT confidence,vendor,cpu_core_seconds
                     FROM legacy.{cpu} WHERE interval_id=l.id)
                   AND NOT EXISTS(SELECT confidence,vendor,cpu_core_seconds FROM legacy.{cpu}
                     WHERE interval_id=l.id EXCEPT SELECT confidence,vendor,cpu_core_seconds
                     FROM main.{cpu} WHERE interval_id=m.id) LIMIT 1"
                ),
                [id],
                |r| r.get(0),
            )
            .optional()?;
        if existing.is_some() {
            continue;
        }
        tx.execute(
            &format!(
                "INSERT INTO main.{parent}(scanner,seq,start_ms,end_ms,status,skipped_processes)
                 SELECT a.mapped,l.seq,l.start_ms,l.end_ms,l.status,l.skipped_processes
                 FROM legacy.{parent} l JOIN temp.attribution_import_scanners a
                   ON a.source=l.scanner WHERE l.id=?1"
            ),
            [id],
        )?;
        let parent_id = tx.last_insert_rowid();
        tx.execute(
            &format!(
                "INSERT INTO main.{cpu}(interval_id,confidence,vendor,cpu_core_seconds)
                 SELECT ?1,confidence,vendor,cpu_core_seconds FROM legacy.{cpu} WHERE interval_id=?2"
            ),
            params![parent_id, id],
        )?;
    }
    Ok(())
}

fn frontiers(tx: &Transaction<'_>, table: &str, version_one: bool) -> JobResult<()> {
    let fields = if version_one {
        "m.seq=l.seq AND m.start_ms=l.start_ms AND m.failed=l.failed"
    } else {
        "m.seq=l.seq AND m.start_ms=l.start_ms AND m.failed=l.failed AND m.low_ms=l.low_ms
         AND m.boot_id IS l.boot_id AND m.boot_ns IS l.boot_ns AND m.sealed=l.sealed"
    };
    let columns = if version_one {
        "seq,start_ms,failed"
    } else {
        "seq,start_ms,failed,low_ms,boot_id,boot_ns,sealed"
    };
    let legacy_values = columns
        .split(',')
        .map(|c| format!("l.{c}"))
        .collect::<Vec<_>>()
        .join(",");
    tx.execute_batch(&format!(
        "INSERT INTO main.{table}(scanner,{columns})
         SELECT a.mapped,{legacy_values} FROM legacy.{table} l
         JOIN temp.attribution_import_scanners a ON a.source=l.scanner
         WHERE NOT EXISTS(SELECT 1 FROM main.{table} m WHERE {fields}
           AND (m.scanner=a.canonical OR EXISTS(
             SELECT 1 FROM main.attribution_merge_scanners x
             WHERE x.source=a.canonical AND x.mapped=m.scanner)))"
    ))?;
    Ok(())
}

pub(super) fn attached(tx: &Transaction<'_>) -> JobResult<()> {
    let names = [
        "attribution_v1_intervals",
        "attribution_v1_interval_cpu",
        "attribution_v1_frontier",
    ];
    let present = names
        .map(|name| schema::table_exists(tx, "legacy", name))
        .into_iter()
        .collect::<JobResult<Vec<_>>>()?;
    if present[1] && !present[0] {
        return Err(JobError::StorageFailure);
    }
    for (here, ddl) in present.iter().zip([V1_INTERVALS, V1_CPU, V1_FRONTIER]) {
        if *here {
            tx.execute_batch(ddl)?;
        }
    }
    // The upgrade normally creates all three v1 tables, but an older file
    // can lack a CPU table. Create an empty one for the comparison query.
    if present[0] && !present[1] {
        tx.execute_batch("CREATE TABLE legacy.attribution_v1_interval_cpu (
            interval_id INTEGER NOT NULL, confidence TEXT NOT NULL, vendor TEXT NOT NULL,
            cpu_core_seconds REAL NOT NULL, PRIMARY KEY(interval_id,confidence,vendor)) WITHOUT ROWID")?;
        tx.execute_batch(V1_CPU)?;
    }
    let mut source = vec!["attribution_intervals", "attribution_frontier"];
    if present[0] {
        source.push(names[0]);
    }
    if present[2] {
        source.push(names[2]);
    }
    let mut target = vec!["attribution_intervals", "attribution_frontier"];
    for name in [names[0], names[2]] {
        if schema::table_exists(tx, "main", name)? {
            target.push(name);
        }
    }
    aliases(tx, &source, &target)?;
    intervals(tx, "attribution_intervals", "attribution_interval_cpu")?;
    frontiers(tx, "attribution_frontier", false)?;
    if present[0] {
        intervals(tx, names[0], names[1])?;
    }
    if present[2] {
        frontiers(tx, names[2], true)?;
    }
    tx.execute_batch(
        "INSERT INTO main.attribution_uncertain(from_ms,to_ms,reason)
         SELECT from_ms,to_ms,reason FROM legacy.attribution_uncertain l
         WHERE NOT EXISTS(SELECT 1 FROM main.attribution_uncertain m
           WHERE m.from_ms=l.from_ms AND m.to_ms=l.to_ms AND m.reason=l.reason);
         DELETE FROM legacy.attribution_interval_cpu;
         DELETE FROM legacy.attribution_intervals;
         DELETE FROM legacy.attribution_frontier;
         DELETE FROM legacy.attribution_uncertain;",
    )?;
    if present[0] {
        tx.execute_batch("DELETE FROM legacy.attribution_v1_interval_cpu; DELETE FROM legacy.attribution_v1_intervals;")?;
    }
    if present[2] {
        tx.execute_batch("DELETE FROM legacy.attribution_v1_frontier")?;
    }
    #[cfg(test)]
    if let Some(path) = std::env::var_os("ARC_ATTRIBUTION_MERGE_CRASH_READY") {
        // The crash regression kills this process after both databases have
        // actually been changed but before SQLite is asked to commit.
        std::fs::write(path, b"both stores changed").map_err(|_| JobError::StorageFailure)?;
        loop {
            std::thread::park();
        }
    }
    // Discard aliases that were unnecessary because all rows were duplicates.
    let mut used = vec![
        "SELECT scanner FROM main.attribution_intervals",
        "SELECT scanner FROM main.attribution_frontier",
    ];
    for name in [names[0], names[2]] {
        if schema::table_exists(tx, "main", name)? {
            used.push(if name == names[0] {
                "SELECT scanner FROM main.attribution_v1_intervals"
            } else {
                "SELECT scanner FROM main.attribution_v1_frontier"
            });
        }
    }
    tx.execute_batch(&format!(
        "DELETE FROM main.attribution_merge_scanners WHERE mapped NOT IN ({})",
        used.join(" UNION ")
    ))?;
    tx.execute_batch("DROP TABLE temp.attribution_import_scanners")?;
    Ok(())
}
