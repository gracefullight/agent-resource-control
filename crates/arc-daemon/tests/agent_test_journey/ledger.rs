//! The real daemon's Traffic records, read from its database.
use rusqlite::Connection;

/// One Traffic entry with its job and, when it owns one, its permit.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub job: String,
    pub class: String,
    pub reason: String,
    pub state: String,
    /// The job whose permit this entry runs under; `None` for a root.
    pub root: Option<String>,
    pub exit: Option<i64>,
    pub permit: Option<String>,
}

/// Every Traffic entry in queue order.
pub fn entries(db: &Connection) -> Vec<Entry> {
    db.prepare(
        "SELECT t.job_id,j.job_class,l.classification_reason,t.state,t.root_job_id,j.exit_code,p.state \
         FROM traffic_entries t JOIN jobs j ON j.id=t.job_id \
         JOIN job_lifecycle l ON l.job_id=t.job_id \
         LEFT JOIN permits p ON p.job_id=t.job_id ORDER BY t.sequence",
    )
    .unwrap()
    .query_map([], |row| {
        Ok(Entry {
            job: row.get(0)?,
            class: row.get(1)?,
            reason: row.get(2)?,
            state: row.get(3)?,
            root: row.get(4)?,
            exit: row.get(5)?,
            permit: row.get(6)?,
        })
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

fn numbers(db: &Connection, sql: &str) -> Vec<i64> {
    db.prepare(sql)
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// Permits that exist, and how many of them are held.
pub fn permits(db: &Connection) -> (i64, i64) {
    (
        numbers(db, "SELECT count(*) FROM permits")[0],
        numbers(db, "SELECT count(*) FROM permits WHERE state='ACTIVE'")[0],
    )
}

/// Record, inside the daemon's own write transactions, the number of held
/// permits after every permit change.
pub fn watch(db: &Connection) {
    db.execute_batch(
        "CREATE TABLE journey_permits(held INTEGER NOT NULL);
         CREATE TRIGGER journey_permit_insert AFTER INSERT ON permits BEGIN
           INSERT INTO journey_permits SELECT count(*) FROM permits WHERE state='ACTIVE'; END;
         CREATE TRIGGER journey_permit_update AFTER UPDATE ON permits BEGIN
           INSERT INTO journey_permits SELECT count(*) FROM permits WHERE state='ACTIVE'; END;",
    )
    .unwrap();
}

/// The held-permit count after each permit change since `watch`, in order.
pub fn held(db: &Connection) -> Vec<i64> {
    numbers(db, "SELECT held FROM journey_permits ORDER BY rowid")
}

/// Remove the observation table so the daemon's schema is as it was.
pub fn unwatch(db: &Connection) {
    db.execute_batch(
        "DROP TRIGGER journey_permit_insert; DROP TRIGGER journey_permit_update;
         DROP TABLE journey_permits;",
    )
    .unwrap();
}
