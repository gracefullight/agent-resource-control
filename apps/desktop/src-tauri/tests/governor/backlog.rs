//! An actual competing SQLite writer, not a delayed/fabricated RPC response.
use super::{native_support::*, serialization, support::*};
use serde_json::json;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};

#[test]
fn gv_n08_sqlite_writer_backlog_discards_prelock_normal_then_pause_releases_once() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    let normal = case.pressure("Normal", 0);
    let mut locker = super::native::OwnedChild::new(
        Command::new("/usr/bin/sqlite3")
            .arg("-bail")
            .arg(&case.fixture.database)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut input = locker.child.stdin.take().unwrap();
    input
        .write_all(b"BEGIN IMMEDIATE;\n.print acquired\n")
        .unwrap();
    input.flush().unwrap();
    let mut line = String::new();
    BufReader::new(locker.child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert_eq!(line.trim(), "acquired");
    let mut run = Run::start(&case, "sqlite-backlog", "build");
    std::fs::write(case.fixture.root.join("pressure"), b"Critical").unwrap();
    let mut critical = None;
    // Policy reads persist bounded current-policy metadata and therefore can
    // report DB_BUSY under this lock. Meter's cached read is independent of
    // SQLite and proves the publication actually changed before unlock.
    eventually(|| {
        let snapshot = case.rpc("meter.snapshot", serde_json::Value::Null);
        if snapshot["host"]["memory_pressure"]["reading"]["value"] == "critical"
            && snapshot["sequence"].as_u64().unwrap()
                > normal["observation"]["sequence"].as_u64().unwrap()
        {
            arc_ipc::meter::decode_snapshot(&snapshot).unwrap();
            critical = Some(snapshot);
            true
        } else {
            false
        }
    });
    let critical = critical.unwrap();
    assert_eq!(run.count(), 0);
    assert!(run.wrapper.child.try_wait().unwrap().is_none());
    assert!(case.rows("SELECT job_id FROM permits").is_empty());
    input.write_all(b"ROLLBACK;\n.quit\n").unwrap();
    input.flush().unwrap();
    drop(input);
    assert!(super::execution::wait(&mut locker.child).success());
    let id = queued(&case, &mut run);
    eventually(|| {
        case.rpc("policy.job", json!({"job_id":id}))["last_decision"]["reason"] == "memory_pressure"
    });
    assert!(
        case.rows("SELECT job_id FROM permits").is_empty(),
        "prelock Normal cannot authorize after fresh Critical"
    );
    let held = case
        .router
        .call("policy_job", json!({"job_id":id}))
        .unwrap();
    let pause = bridge_set(&case, &intent(0, false, 0x61));
    let child = run.started();
    assert_eq!(run.finish().code(), Some(17));
    let done = job(&case, &id, &run, &child, true);
    let final_permits = permits(&case, 0, 0);
    save_case(
        "GV-N08-writer-backlog",
        json!({"lockHandshake":line,"beforeBacklog":normal,"freshWhileLocked":critical,
        "heldAfterUnlock":held,"pause":pause,"outcome":done,"actualChildren":run.count(),"permits":final_permits}),
    );
    case.stop();
}
