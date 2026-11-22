//! TRF-015 item 3 through built `arcctl job diagnostics` and a real daemon.
use super::fixture::*;
use crate::{
    journey::{eventually, queue_state, Sandbox},
    native_exec,
    runtime::{child_watchdog::Owned, Fixture},
};
use serde_json::{json, Value};
use std::{io::Write, os::unix::net::UnixListener, process::Stdio};

/// The history SQLite holds for `job`, in the reply's shape.
fn recorded(db: &rusqlite::Connection, job: &str) -> Value {
    Value::Array(
        decisions(db, job)
            .into_iter()
            .enumerate()
            .map(|(index, (kind, reason, at, key))| {
                let reason = if key.starts_with("[\"start_rate\"") {
                    "start_rate".to_owned()
                } else {
                    reason
                };
                json!({"index": index + 1, "decided_at_ms": at, "kind": kind, "reason": reason})
            })
            .collect(),
    )
}

/// The `Decision history:` block the text output must contain for `history`.
fn block(history: &Value) -> String {
    let mut text = String::from("Decision history:\n");
    for entry in history.as_array().unwrap() {
        text.push_str(&format!(
            "  {}. {} {} {}",
            entry["index"],
            entry["decided_at_ms"],
            entry["kind"].as_str().unwrap(),
            entry["reason"].as_str().unwrap()
        ));
        text.push_str(match entry["reason"].as_str().unwrap() {
            "class_window" => " (waiting for a class window)\n",
            "fifo" => " (waiting for its turn in the queue)\n",
            "cpu_budget" => " (waiting for the CPU budget)\n",
            "memory_budget" => " (waiting for the memory budget)\n",
            "memory_pressure" => " (held under memory pressure)\n",
            "start_rate" => " (waiting for a start-rate token)\n",
            _ => "\n",
        });
    }
    text
}

pub fn diagnostics_reads_every_decision_in_order_and_schedule() {
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    s.stub("cargo");
    let (held_project, project) = (s.project("held"), s.project("project"));
    let marker = f.root.join("starts");
    let daemon = daemon(&f, &[]);
    let db = database(&f);

    // A separate build holds the one Build permit at its child's barrier.
    let control = f.root.join("held-build");
    let listener = UnixListener::bind(&control).unwrap();
    let mut holder = cargo(&f, &s, &held_project, "build", &f.root.join("held-starts"));
    holder
        .env("ARC_FIXTURE_CONTROL", &control)
        .stderr(Stdio::null());
    let mut holder = Owned::spawn(&mut holder).unwrap();
    let (mut release, _) = native_exec::accept(&listener);

    let mut waiter = cargo(&f, &s, &project, "build", &marker);
    let mut waiter = Owned::spawn(waiter.stderr(Stdio::null())).unwrap();
    let pid = waiter.child.id();
    eventually("the waiter queued with a decision", || {
        queue_state(&db, pid).as_deref() == Some("QUEUED")
            && !decisions(&db, &job_of(&db, pid)).is_empty()
    });
    let job = job_of(&db, pid);

    // While queued: every decision so far, its queue position, and no
    // invented schedule for a wait whose end the daemon cannot derive.
    let document = diagnostics_json(&f, &s, &job);
    let history = &document["decision_history"];
    let queued = recorded(&db, &job);
    assert_eq!(history["decisions"], queued, "{document}");
    assert_eq!(queued[0]["kind"], "wait", "{queued}");
    assert_eq!(history["queue_position"], json!([1, 1]));
    assert_eq!(history["expected_schedule_ms"], Value::Null);
    let text = diagnostics(&f, &s, &job, false);
    assert!(text.contains("\nLast recorded decision: "), "{text}");
    assert!(
        text.contains(&format!(
            "{}Queue position: 1 of 1\nExpected schedule: unknown\n",
            block(&queued)
        )),
        "{text}"
    );

    // After the release the waiter is granted and runs: the history keeps
    // the wait and adds every later decision, in recorded order.
    release.write_all(b"x").unwrap();
    assert!(holder.wait("held build").unwrap().success());
    assert!(waiter.wait("waiter").unwrap().success());
    eventually("the waiter completed", || {
        queue_state(&db, pid).as_deref() == Some("COMPLETED")
    });
    let document = diagnostics_json(&f, &s, &job);
    let history = &document["decision_history"];
    let all = recorded(&db, &job);
    assert_eq!(history["decisions"], all, "{document}");
    assert!(
        all.as_array().unwrap().len() > queued.as_array().unwrap().len(),
        "{all}"
    );
    assert_eq!(all[0], queued[0], "the first decision is kept");
    assert!(
        all.as_array().unwrap().iter().any(|d| d["kind"] == "admit"),
        "{all}"
    );
    let times: Vec<u64> = all
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["decided_at_ms"].as_u64().unwrap())
        .collect();
    assert!(times.windows(2).all(|t| t[0] <= t[1]), "{times:?}");
    assert_eq!(history["queue_position"], Value::Null);
    let text = diagnostics(&f, &s, &job, false);
    assert!(
        text.ends_with(&format!("{}Queue position: not queued\n", block(&all))),
        "{text}"
    );
    drop(db);
    stop(&f, daemon);
}
