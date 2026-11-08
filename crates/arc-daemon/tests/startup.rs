mod support;

use arc_storage::open_and_migrate;
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

struct TempDatabase(PathBuf);

impl TempDatabase {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let directory = arc_core::runtime_root().unwrap().join(format!(
            "d{:x}-{:x}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        Self(directory.join("arc.sqlite3"))
    }
}

impl Drop for TempDatabase {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0.parent().unwrap());
    }
}

fn spawn_daemon(path: &std::path::Path, socket: &std::path::Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_arc-daemon"))
        .env(arc_core::ARC_DB_PATH, path)
        .env(arc_core::ARC_SOCKET_PATH, socket)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

fn wait_for_exit(mut child: Child, timeout: Duration) -> Output {
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!("daemon did not exit within the deadline: {output:?}");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn checksum_rejection_exits_nonzero_with_recovery_guidance() {
    let _serial = support::serial();
    let database = TempDatabase::new();
    let socket = database.0.parent().unwrap().join("arc.sock");
    let connection = open_and_migrate(&database.0).unwrap();
    connection
        .execute("UPDATE schema_migrations SET checksum='invalid'", [])
        .unwrap();
    connection
        .execute(
            "INSERT INTO vendors VALUES ('keep','Existing record',NULL,42)",
            [],
        )
        .unwrap();
    let before = database_snapshot(&connection);
    let child = spawn_daemon(&database.0, &socket);
    let output = wait_for_exit(child, Duration::from_secs(10));
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "Database startup failed: migration checksum mismatch; preserve the database and investigate against a trusted copy.\n"
    );
    assert!(!socket.exists());
    assert_eq!(database_snapshot(&connection), before);
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM gc_operations", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row("SELECT checksum FROM schema_migrations", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        "invalid"
    );
}

#[test]
fn missing_invalid_and_newer_schema_startup_reject_without_data_or_history_changes() {
    let _serial = support::serial();
    for (sql, reason) in [
        ("DELETE FROM schema_migrations", "missing migration history; preserve the database and investigate against a trusted copy"),
        ("UPDATE schema_migrations SET id='0001_other' WHERE id='0001_phase0'", "invalid migration history; preserve the database and investigate against a trusted copy"),
        ("INSERT INTO schema_migrations VALUES ('0003_future',42,'future-checksum'); PRAGMA user_version=3", "unsupported newer schema version 3 (supported 2); use a binary supporting that version; downgrade execution is not supported"),
    ] {
        let database = TempDatabase::new();
        let socket = database.0.parent().unwrap().join("arc.sock");
        let connection = open_and_migrate(&database.0).unwrap();
        connection.execute("INSERT INTO vendors VALUES ('keep','Existing record',NULL,42)", []).unwrap();
        connection.execute_batch(sql).unwrap();
        let before = database_snapshot(&connection);
        let output = wait_for_exit(spawn_daemon(&database.0, &socket), Duration::from_secs(10));
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(String::from_utf8(output.stderr).unwrap(), format!("Database startup failed: {reason}.\n"));
        assert!(!socket.exists(), "rejected startup must not advertise doctor");
        assert_eq!(database_snapshot(&connection), before);
        connection.close().unwrap();
    }
}

fn database_snapshot(connection: &rusqlite::Connection) -> (String, String, String, u32) {
    // SQLite JSON aggregation preserves complete values without line ambiguities.
    let query = |sql| {
        connection
            .query_row(sql, [], |r| r.get::<_, String>(0))
            .unwrap()
    };
    (
        query("SELECT json_group_array(json_array(id,display_name,adapter_version,discovered_at)) FROM (SELECT * FROM vendors ORDER BY id)"),
        query("SELECT json_group_array(json_array(id,applied_at,checksum)) FROM (SELECT * FROM schema_migrations ORDER BY id)"),
        query("SELECT json_group_array(json_array(type,name,sql)) FROM (SELECT * FROM sqlite_schema ORDER BY type,name)"),
        connection.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap(),
    )
}
