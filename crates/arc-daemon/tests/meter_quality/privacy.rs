//! Session privacy of the daemon that handled the session events (G-01
//! item 3): its IPC-visible diagnostics while it runs, then its own stdout,
//! stderr, runtime report and database after it stops and before any
//! restart, whose fresh output files would truncate the evidence.
use crate::{
    journey::Sandbox,
    runtime::{child_watchdog, Fixture},
};
use rusqlite::{types::ValueRef, Connection};
use serde_json::Value;
use std::{fs, path::Path};

/// Where the original daemon's output is preserved for the final scan.
pub const ORIGINAL_LOGS: [(&str, &str); 2] = [
    ("stdout", "original-daemon-stdout"),
    ("stderr", "original-daemon-stderr"),
];

fn occurs(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

/// Every raw identifier the delivered hook payloads carried: each session
/// id (both recorded session texts) and each tool call id.
fn raw_ids(events: &[Value]) -> Vec<String> {
    let mut ids: Vec<String> = events
        .iter()
        .flat_map(|event| ["session_id", "tool_use_id"].map(|k| event[k].as_str().unwrap()))
        .map(str::to_owned)
        .collect();
    ids.sort();
    ids.dedup();
    for session in [crate::sessions::SESSION_A, crate::sessions::SESSION_B] {
        assert!(
            ids.iter().any(|id| id == session),
            "{session} not delivered"
        );
    }
    ids
}

/// Fail if any raw id is in `bytes`. The same scan must find each id when
/// it is placed inside these exact bytes, so an empty, unreadable or
/// mis-decoded source cannot pass vacuously.
#[track_caller]
fn assert_clean(label: &str, bytes: &[u8], ids: &[String]) {
    for id in ids {
        assert!(!occurs(bytes, id.as_bytes()), "raw id {id} is in {label}");
        let mid = bytes.len() / 2;
        let planted = [&bytes[..mid], id.as_bytes(), &bytes[mid..]].concat();
        assert!(occurs(&planted, id.as_bytes()), "{label} scan is blind");
    }
}

/// The session jobs the original daemon recorded, with their digests.
pub fn session_jobs(db: &Connection) -> Vec<(String, String)> {
    let jobs: Vec<(String, String)> = db
        .prepare("SELECT id,session_key FROM jobs WHERE session_key IS NOT NULL ORDER BY rowid")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(jobs.len(), 4, "{jobs:?}");
    jobs
}

/// While the original daemon runs: the built CLI's job status, scheduling
/// diagnostics and job meter report of every session job, read over IPC,
/// name the job and carry no raw id, in their output or their errors.
pub fn assert_diagnostics_hide_raw_ids(
    f: &Fixture,
    s: &Sandbox,
    project: &Path,
    jobs: &[(String, String)],
    events: &[Value],
) {
    let ids = raw_ids(events);
    for (job, _) in jobs {
        for args in [
            vec!["job", "status", job.as_str()],
            vec!["job", "diagnostics", job.as_str(), "--json"],
            vec![
                "meter",
                "summary",
                "--job",
                job.as_str(),
                "--footprint",
                "--json",
            ],
        ] {
            let label = args.join(" ");
            let output = child_watchdog::output(s.arcctl(f, project).args(&args), &label).unwrap();
            let bytes = [output.stdout.as_slice(), output.stderr.as_slice()].concat();
            assert!(output.status.success(), "{label}: {output:?}");
            assert!(
                occurs(&output.stdout, job.as_bytes()),
                "{label} names no job"
            );
            assert_clean(&label, &bytes, &ids);
        }
    }
}

/// Every stored value of every table, as bytes, read over `db`. The files
/// are not read directly here: closing another descriptor on the database
/// or its WAL index would drop this process's SQLite locks while `db` is
/// open, and the next daemon would then rebuild the index under it. The
/// raw bytes, free pages included, are scanned at the end, after `db` is
/// closed and no daemon runs.
fn stored(db: &Connection) -> Vec<u8> {
    let tables: Vec<String> = db
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut bytes = Vec::new();
    for table in tables {
        let mut statement = db
            .prepare(&format!("SELECT * FROM \"{}\"", table.replace('"', "\"\"")))
            .unwrap();
        let columns = statement.column_count();
        let mut rows = statement.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for column in 0..columns {
                match row.get_ref(column).unwrap() {
                    ValueRef::Text(value) | ValueRef::Blob(value) => bytes.extend_from_slice(value),
                    _ => {}
                }
                bytes.push(0);
            }
        }
    }
    bytes
}

/// After the original daemon stopped and before the first restart: its
/// stdout and stderr (which a restart recreates empty), its runtime report
/// and every value it stored carry no raw id, while the logs are that
/// daemon's own (they record its readiness) and the store holds each
/// session job's digest. The logs are then kept beside the fresh ones so
/// the final whole-fixture scan reads them again.
pub fn assert_original_daemon_kept_no_raw_ids(
    f: &Fixture,
    db: &Connection,
    jobs: &[(String, String)],
    events: &[Value],
) {
    let ids = raw_ids(events);
    let mut logs = Vec::new();
    for (name, _) in ORIGINAL_LOGS {
        let bytes = fs::read(f.root.join(name)).unwrap();
        assert_clean(&format!("original daemon {name}"), &bytes, &ids);
        logs.extend_from_slice(&bytes);
    }
    assert!(
        occurs(&logs, b"arc-daemon ready."),
        "not the daemon's output"
    );
    let report = fs::read(f.root.join("runtime.json")).unwrap();
    assert_clean("original runtime report", &report, &ids);
    let database = stored(db);
    assert_clean("stored values", &database, &ids);
    for (job, digest) in jobs {
        assert!(
            occurs(&database, digest.as_bytes()),
            "{job} digest not stored"
        );
    }
    for (name, kept) in ORIGINAL_LOGS {
        fs::rename(f.root.join(name), f.root.join(kept)).unwrap();
    }
}
