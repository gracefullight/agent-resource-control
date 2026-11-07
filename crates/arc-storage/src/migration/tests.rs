use super::*;
use rusqlite::types::Value;
use std::{
    cell::RefCell,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

mod acceptance;
mod concurrency;
mod rejection;

const FIXTURES: &[Migration] = &[
    Migration {
        version: 1,
        id: "0001_fixture",
        sql: include_str!("../../tests/fixtures/0001_fixture.sql"),
    },
    Migration {
        version: 2,
        id: "0002_additive",
        sql: include_str!("../../tests/fixtures/0002_additive.sql"),
    },
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    Locked,
    BeforeCommit,
}
type Observer = Box<dyn FnMut(Stage, &Connection)>;
thread_local! {
    static OBSERVER: RefCell<Option<Observer>> = RefCell::new(None);
}
pub(super) fn observe(stage: Stage, connection: &Connection) {
    OBSERVER.with_borrow_mut(|observer| {
        if let Some(observer) = observer {
            observer(stage, connection);
        }
    });
}
fn observer(callback: impl FnMut(Stage, &Connection) + 'static) {
    OBSERVER.with_borrow_mut(|slot| *slot = Some(Box::new(callback)));
}

struct Database(PathBuf);
impl Database {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "arc-migration-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        Self(directory.join("db.sqlite3"))
    }
    fn connect(&self) -> Connection {
        crate::open_connection(&self.0).unwrap()
    }
    fn v1(&self) -> Connection {
        let mut connection = self.connect();
        migrate(&mut connection, &FIXTURES[..1]).unwrap();
        connection
            .execute("INSERT INTO vendors VALUES ('kept', 'original')", [])
            .unwrap();
        connection
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0.parent().unwrap());
    }
}

type History = Vec<(String, i64, String)>;
fn history(connection: &Connection) -> History {
    connection
        .prepare("SELECT id, applied_at, checksum FROM schema_migrations ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[derive(Debug, PartialEq)]
struct Snapshot {
    schema: Vec<(String, String, String, Option<String>)>,
    tables: Vec<(String, Vec<Vec<Value>>)>,
    witness: i64,
}
fn snapshot(connection: &Connection) -> Snapshot {
    let schema: Vec<(String, String, String, Option<String>)> = connection
        .prepare("SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY type, name")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let tables = schema
        .iter()
        .filter(|object| object.0 == "table")
        .map(|object| {
            let name = &object.1;
            let mut statement = connection
                .prepare(&format!(
                    "SELECT * FROM \"{}\" ORDER BY rowid",
                    name.replace('"', "\"\"")
                ))
                .unwrap();
            let columns = statement.column_count();
            let rows = statement
                .query_map([], |row| (0..columns).map(|index| row.get(index)).collect())
                .unwrap()
                .collect::<Result<Vec<Vec<Value>>, _>>()
                .unwrap();
            (name.clone(), rows)
        })
        .collect();
    Snapshot {
        schema,
        tables,
        witness: connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap(),
    }
}

fn reject_unchanged(connection: &mut Connection, registry: &[Migration], status: SchemaStatus) {
    let before = snapshot(connection);
    let assessment = assess_with_registry(connection, registry).unwrap();
    assert_eq!(assessment.status, status);
    assert!(!assessment.history_verified);
    assert_eq!(snapshot(connection), before, "assessment must be read-only");
    match migrate(connection, registry).unwrap_err() {
        MigrationError::Incompatible(actual) => assert_eq!(actual, assessment),
        error => panic!("unexpected error: {error}"),
    }
    assert!(connection.is_autocommit());
    assert_eq!(
        snapshot(connection),
        before,
        "rejected open changed database"
    );
}
