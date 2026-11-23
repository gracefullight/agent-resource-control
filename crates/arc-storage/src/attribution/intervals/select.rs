//! A window's selection, read in one snapshot of each store file and
//! combined without counting an interval twice.
use super::{schema, sql, Frontier, Open, Selection};
use crate::jobs::{JobError, JobResult};
use rusqlite::{params, Connection};
use std::collections::{BTreeMap, BTreeSet};

type Key = (String, i64);
type FrontierShape = (u64, u64, u64, bool, bool, Option<super::BootClock>);

#[derive(Debug, PartialEq)]
struct Sample {
    start: u64,
    end: u64,
    skipped: u64,
    cpu: BTreeMap<(String, String), f64>,
}

/// What one store file holds for a window.
#[derive(Debug, Default)]
pub(super) struct Raw {
    /// Measured intervals wholly inside: bounds and skipped processes.
    measured: BTreeMap<i64, (Key, Sample)>,
    /// Failed intervals overlapping.
    failed: BTreeSet<Key>,
    open: Vec<Open>,
    /// A range of version-1 bounds overlaps (or the file is version 1).
    uncertain: bool,
}

fn version(connection: &Connection) -> JobResult<i64> {
    Ok(connection.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// The durable frontiers of one file (current, version-3 or version-2
/// layout). A version-3 boot reading at its cap is not comparable: none.
pub(super) fn frontiers(connection: &Connection) -> JobResult<Vec<Frontier>> {
    let capped = schema::CAPPED_BOOT_NS;
    let query = match version(connection)? {
        0 | 1 => return Ok(Vec::new()),
        2 => "SELECT start_ms,start_ms,NULL,NULL FROM attribution_frontier".to_owned(),
        3 => format!(
            "SELECT start_ms,low_ms,
               CASE WHEN boot_ns>={capped} THEN NULL ELSE boot_id END,
               CASE WHEN boot_ns>={capped} THEN NULL ELSE boot_ns END
             FROM attribution_frontier"
        ),
        schema::SCHEMA_VERSION => {
            "SELECT start_ms,low_ms,boot_id,boot_ns FROM attribution_frontier".to_owned()
        }
        _ => return Err(JobError::StorageFailure),
    };
    let mut statement = connection.prepare(&query)?;
    let rows = statement.query_map([], |r| {
        let boot: Option<(String, i64)> = match (r.get(2)?, r.get(3)?) {
            (Some(id), Some(ns)) => Some((id, ns)),
            _ => None,
        };
        Ok(Frontier {
            start_ms: r.get::<_, i64>(0)? as u64,
            low_ms: r.get::<_, i64>(1)? as u64,
            boot: boot.map(|(boot_id, ns)| super::BootClock {
                boot_id,
                ns: ns.max(0) as u64,
            }),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub(super) fn read(connection: &mut Connection, from_ms: u64, to_ms: u64) -> JobResult<Raw> {
    let version = version(connection)?;
    let mut raw = Raw::default();
    match version {
        0 => return Ok(raw),
        // Completion bounds no window can trust; nothing is read from them.
        1 => {
            raw.uncertain = true;
            return Ok(raw);
        }
        2 | 3 | schema::SCHEMA_VERSION => (),
        _ => return Err(JobError::StorageFailure),
    }
    let tx = connection.transaction()?;
    let range = params![sql(from_ms), sql(to_ms)];
    let has_aliases: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='attribution_merge_scanners')",
        [],
        |r| r.get(0),
    )?;
    let scanner = if has_aliases {
        "COALESCE((SELECT a.source FROM attribution_merge_scanners a WHERE a.mapped=i.scanner),i.scanner)"
    } else {
        "i.scanner"
    };
    {
        let mut statement = tx.prepare(&format!(
            "SELECT i.id,{scanner},i.seq,i.start_ms,i.end_ms,i.skipped_processes FROM attribution_intervals i
             WHERE i.status='measured' AND i.start_ms>=?1 AND i.end_ms<=?2"
        ))?;
        let mut rows = statement.query(range)?;
        while let Some(r) = rows.next()? {
            raw.measured.insert(
                r.get(0)?,
                (
                    (r.get(1)?, r.get(2)?),
                    Sample {
                        start: r.get::<_, i64>(3)? as u64,
                        end: r.get::<_, i64>(4)? as u64,
                        skipped: r.get::<_, i64>(5)?.max(0) as u64,
                        cpu: BTreeMap::new(),
                    },
                ),
            );
        }
    }
    {
        let mut statement = tx.prepare(
            "SELECT i.id,c.confidence,c.vendor,c.cpu_core_seconds
             FROM attribution_interval_cpu c JOIN attribution_intervals i ON i.id=c.interval_id
             WHERE i.status='measured' AND i.start_ms>=?1 AND i.end_ms<=?2",
        )?;
        let mut rows = statement.query(range)?;
        while let Some(r) = rows.next()? {
            if let Some((_, sample)) = raw.measured.get_mut(&r.get::<_, i64>(0)?) {
                sample.cpu.insert((r.get(1)?, r.get(2)?), r.get(3)?);
            }
        }
    }
    {
        let mut statement = tx.prepare(
            "SELECT scanner,seq FROM attribution_intervals
             WHERE status='failed' AND start_ms<?2 AND end_ms>?1",
        )?;
        let mut rows = statement.query(range)?;
        while let Some(r) = rows.next()? {
            raw.failed.insert((r.get(0)?, r.get(1)?));
        }
    }
    // Read all frontiers: a conflicting later frontier can make an earlier
    // same-identity frontier ambiguous even when its own floor is outside
    // this particular window.
    let (low, sealed, boot_id, boot_ns) = match version {
        schema::SCHEMA_VERSION => (
            "i.low_ms",
            "i.sealed",
            "i.boot_id".to_owned(),
            "i.boot_ns".to_owned(),
        ),
        3 => (
            "i.low_ms",
            "0",
            format!(
                "CASE WHEN i.boot_ns>={} THEN NULL ELSE i.boot_id END",
                schema::CAPPED_BOOT_NS
            ),
            format!(
                "CASE WHEN i.boot_ns>={} THEN NULL ELSE i.boot_ns END",
                schema::CAPPED_BOOT_NS
            ),
        ),
        _ => ("i.start_ms", "0", "NULL".to_owned(), "NULL".to_owned()),
    };
    {
        let query = format!("SELECT i.scanner,{scanner},i.seq,i.start_ms,{low},i.failed,{sealed},{boot_id},{boot_ns} FROM attribution_frontier i");
        let mut statement = tx.prepare(&query)?;
        let rows = statement.query_map([], |r| {
            let boot: Option<(String, i64)> = match (r.get(7)?, r.get(8)?) {
                (Some(id), Some(ns)) => Some((id, ns)),
                _ => None,
            };
            Ok(Open {
                scanner: r.get(0)?,
                origin: r.get(1)?,
                seq: r.get::<_, i64>(2)? as u64,
                start_ms: r.get::<_, i64>(3)? as u64,
                low_ms: r.get::<_, i64>(4)? as u64,
                failed: r.get::<_, i64>(5)? != 0,
                sealed: r.get::<_, i64>(6)? != 0,
                boot: boot.map(|(boot_id, ns)| super::BootClock {
                    boot_id,
                    ns: ns.max(0) as u64,
                }),
                conflicted: false,
            })
        })?;
        raw.open = rows.collect::<Result<_, _>>()?;
    }
    if version >= 3 {
        raw.uncertain = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM attribution_uncertain WHERE from_ms<?2 AND to_ms>?1)",
            range,
            |r| r.get(0),
        )?;
    }
    Ok(raw)
}

/// One window over every store file read; an interval in two files counts
/// once.
pub(super) fn combine(raws: Vec<Raw>) -> Selection {
    let mut measured: BTreeMap<Key, Vec<Sample>> = BTreeMap::new();
    let mut failed = BTreeSet::new();
    let mut selection = Selection::default();
    for raw in raws {
        for (_, (key, sample)) in raw.measured {
            let rows = measured.entry(key).or_default();
            if !rows.contains(&sample) {
                rows.push(sample);
            }
        }
        failed.extend(raw.failed);
        selection.open.extend(raw.open);
        selection.uncertain |= raw.uncertain;
    }
    let mut histories: BTreeMap<String, Vec<FrontierShape>> = BTreeMap::new();
    for open in &selection.open {
        let shape = (
            open.seq,
            open.start_ms,
            open.low_ms,
            open.failed,
            open.sealed,
            open.boot.clone(),
        );
        let shapes = histories.entry(open.origin.clone()).or_default();
        if !shapes.contains(&shape) {
            shapes.push(shape);
        }
    }
    for open in &mut selection.open {
        open.conflicted = histories
            .get(&open.origin)
            .is_some_and(|shapes| shapes.len() > 1);
    }
    let mut bounds: Vec<(u64, u64)> = measured
        .values()
        .flatten()
        .map(|s| (s.start, s.end))
        .collect();
    bounds.sort_unstable();
    let mut reach: Option<u64> = None;
    for (start, end) in bounds {
        selection.intervals += 1;
        selection.first_start_ms.get_or_insert(start);
        selection.last_end_ms = Some(selection.last_end_ms.map_or(end, |l| l.max(end)));
        // The union of envelopes ordered by start.
        let from = reach.map_or(start, |r| r.max(start));
        selection.covered_ms += end.saturating_sub(from);
        reach = Some(reach.map_or(end, |r| r.max(end)));
    }
    selection.skipped = measured
        .values()
        .flatten()
        .fold(0u64, |sum, sample| sum.saturating_add(sample.skipped));
    selection.failed = failed.len() as u64;
    let mut groups: BTreeMap<(String, String), f64> = BTreeMap::new();
    for sample in measured.into_values().flatten() {
        for (group, seconds) in sample.cpu {
            *groups.entry(group).or_default() += seconds;
        }
    }
    selection.groups = groups
        .into_iter()
        .map(|((confidence, vendor), seconds)| (confidence, vendor, seconds))
        .collect();
    selection
}
