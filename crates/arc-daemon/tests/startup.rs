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
fn migration_failure_exits_nonzero_with_one_stderr_sentence() {
    let _serial = support::serial();
    let database = TempDatabase::new();
    let socket = database.0.parent().unwrap().join("arc.sock");
    let connection = open_and_migrate(&database.0).unwrap();
    connection
        .execute("UPDATE schema_migrations SET checksum='invalid'", [])
        .unwrap();
    let child = spawn_daemon(&database.0, &socket);
    let output = wait_for_exit(child, Duration::from_secs(10));
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "Database startup failed: Phase 0 migration checksum is missing or mismatched.\n"
    );
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
