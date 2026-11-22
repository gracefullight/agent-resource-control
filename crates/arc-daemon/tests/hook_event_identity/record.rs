//! What the real daemon recorded about hook events, read from its database,
//! and what the fixture directory holds on disk.
use crate::runtime::Fixture;
use rusqlite::Connection;
use std::{fs, path::Path};

/// Every job with the event key it recorded, in registration order.
pub fn keys(db: &Connection) -> Vec<(String, Option<String>)> {
    db.prepare("SELECT job_id,event_key FROM job_lifecycle ORDER BY rowid")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// The job each recorded duplicate delivery names, in delivery order. Every
/// such row is `INFO`, carries no payload and has the documented id shape.
pub fn duplicates(db: &Connection) -> Vec<String> {
    db.prepare(
        "SELECT id,severity,entity_id,payload_redacted_json FROM audit_events \
         WHERE event_type='HOOK_EVENT_DUPLICATE' ORDER BY rowid",
    )
    .unwrap()
    .query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })
    .unwrap()
    .map(|row| {
        let (id, severity, job, payload) = row.unwrap();
        assert_eq!((severity.as_str(), &payload), ("INFO", &None), "{id}");
        let delivery = id
            .strip_prefix(&format!("hook-event-duplicate-{job}-"))
            .unwrap_or_else(|| panic!("{id}"));
        assert!(delivery.len() == 32 && delivery != job, "{id}");
        job
    })
    .collect()
}

/// Rows of the tables an admitted job occupies: jobs, lifecycle records,
/// queue entries, negotiated dispositions and permits.
pub fn rows(db: &Connection) -> [i64; 5] {
    [
        "jobs",
        "job_lifecycle",
        "traffic_entries",
        "governor_jobs",
        "permits",
    ]
    .map(|table| {
        db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    })
}

fn files(directory: &Path, found: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let kind = fs::symlink_metadata(&path).unwrap().file_type();
        if kind.is_dir() {
            files(&path, found);
        } else if kind.is_file() {
            found.push(path);
        }
    }
}

/// Neither raw id of `event` is in any file of the fixture directory: the
/// database with its journal, the daemon's output and runtime report, the
/// vendor settings, and every stream and observation of the executions. The
/// test itself never writes the event to disk.
#[track_caller]
pub fn assert_no_raw_ids(f: &Fixture, event: &serde_json::Value) {
    let mut found = Vec::new();
    files(&f.root, &mut found);
    let database = f.database.file_name().unwrap().to_str().unwrap();
    assert!(
        found.iter().any(|path| path.ends_with(database)),
        "{found:?}"
    );
    for name in ["session_id", "tool_use_id"] {
        let raw = event[name].as_str().unwrap().as_bytes();
        assert!(raw.len() >= 8, "{name}");
        for path in &found {
            let bytes = fs::read(path).unwrap();
            assert!(
                !bytes.windows(raw.len()).any(|window| window == raw),
                "{name} is in {}",
                path.display()
            );
        }
    }
}
