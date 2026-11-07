use super::*;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

const WAIT: Duration = Duration::from_secs(10);
thread_local! {
    static BUSY: RefCell<Option<(Sender<()>, Receiver<()>)>> = const { RefCell::new(None) };
}
fn busy_handshake(_: i32) -> bool {
    BUSY.with_borrow_mut(|slot| {
        let (entered, proceed) = slot.take().expect("one blocked lock acquisition");
        entered.send(()).unwrap();
        proceed.recv_timeout(WAIT).is_ok()
    })
}

#[test]
fn concurrent_upgraders_serialize_and_reread_after_lock_acquisition() {
    let database = Database::new();
    drop(database.v1());
    let mut first = database.connect();
    let mut second = database.connect();
    let (locked_tx, locked_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first_thread = thread::spawn(move || {
        observer(move |stage, _| {
            if stage == Stage::Locked {
                locked_tx.send(()).unwrap();
                release_rx.recv_timeout(WAIT).unwrap();
            }
        });
        migrate(&mut first, FIXTURES).unwrap();
        snapshot(&first)
    });
    locked_rx.recv_timeout(WAIT).unwrap();
    let (busy_tx, busy_rx) = mpsc::channel();
    let (retry_tx, retry_rx) = mpsc::channel();
    let (reread_tx, reread_rx) = mpsc::channel();
    let second_thread = thread::spawn(move || {
        BUSY.with_borrow_mut(|slot| *slot = Some((busy_tx, retry_rx)));
        second.busy_handler(Some(busy_handshake)).unwrap();
        observer(move |stage, connection| {
            if stage == Stage::Locked {
                reread_tx.send(history(connection)).unwrap();
            }
        });
        migrate(&mut second, FIXTURES).unwrap();
        snapshot(&second)
    });
    // SQLite itself reports that upgrader B tried the write lock held by A.
    busy_rx.recv_timeout(WAIT).unwrap();
    release_tx.send(()).unwrap();
    let committed = first_thread.join().unwrap();
    retry_tx.send(()).unwrap();
    let reread = reread_rx.recv_timeout(WAIT).unwrap();
    assert_eq!(
        reread.len(),
        2,
        "B must see A's committed migration history"
    );
    assert_eq!(
        second_thread.join().unwrap(),
        committed,
        "B reapplied an upgrade"
    );
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn process_interruption_before_commit_rolls_back_and_recovers() {
    let database = Database::new();
    let connection = database.v1();
    let before = snapshot(&connection);
    let mut child = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "migration::tests::concurrency::interrupted_child",
                "--nocapture",
            ])
            .env("ARC_MIGRATION_INTERRUPTED_CHILD", &database.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if line.unwrap() == "ARC_PENDING_COMMIT" {
                ready_tx.send(()).unwrap();
                return;
            }
        }
    });
    ready_rx
        .recv_timeout(WAIT)
        .expect("child must reach known pre-commit boundary");
    assert_eq!(
        snapshot(&connection),
        before,
        "uncommitted changes leaked to reader"
    );
    child.0.kill().unwrap();
    assert!(!child.0.wait().unwrap().success());
    reader.join().unwrap();
    connection.close().unwrap();
    let mut recovered = database.connect();
    assert_eq!(
        snapshot(&recovered),
        before,
        "interruption left pending state"
    );
    migrate(&mut recovered, FIXTURES).unwrap();
    let committed = snapshot(&recovered);
    assert_eq!(committed.witness, 2);
    assert_eq!(history(&recovered).len(), 2);
    migrate(&mut recovered, FIXTURES).unwrap();
    assert_eq!(snapshot(&recovered), committed);
}

#[test]
fn interrupted_child() {
    let Some(path) = std::env::var_os("ARC_MIGRATION_INTERRUPTED_CHILD") else {
        return;
    };
    let mut connection = crate::open_connection(std::path::Path::new(&path)).unwrap();
    observer(|stage, connection| {
        if stage == Stage::BeforeCommit {
            assert_eq!(history(connection).len(), 2);
            assert_eq!(snapshot(connection).witness, 2);
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM migration_notes", [], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 1);
            std::io::stdout()
                .write_all(b"\nARC_PENDING_COMMIT\n")
                .unwrap();
            std::io::stdout().flush().unwrap();
            let mut buffer = String::new();
            std::io::stdin().read_line(&mut buffer).unwrap();
            panic!("parent must terminate the pending transaction");
        }
    });
    migrate(&mut connection, FIXTURES).unwrap();
    panic!("interrupted upgrade unexpectedly committed");
}
