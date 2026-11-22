use super::*;
use arc_ipc::traffic::decision_history::{HistoryEntry, SCHEMA_VERSION};

fn history(queue: Option<(u64, u64)>, schedule: Option<u64>) -> DecisionHistory {
    DecisionHistory {
        schema_version: SCHEMA_VERSION,
        job_id: JobId::try_from("0123456789abcdef0123456789abcdef".to_owned()).unwrap(),
        decisions: vec![
            HistoryEntry {
                index: 1,
                decided_at_ms: 10,
                kind: "wait".into(),
                reason: "class_window".into(),
            },
            HistoryEntry {
                index: 2,
                decided_at_ms: 20,
                kind: "wait".into(),
                reason: "start_rate".into(),
            },
        ],
        queue_position: queue,
        expected_schedule_ms: schedule,
        policy: None,
    }
}

#[test]
fn every_decision_is_listed_in_order_with_its_schedule() {
    assert_eq!(lines(None), "");
    assert_eq!(
        lines(Some(&history(Some((1, 3)), Some(99)))),
        "Decision history:\n  1. 10 wait class_window (waiting for a class window)\n  \
         2. 20 wait start_rate (waiting for a start-rate token)\nQueue position: 1 of 3\n\
         Expected schedule: start-rate token at 99 (Unix ms)\n"
    );
    assert!(lines(Some(&history(Some((2, 2)), None))).ends_with("Expected schedule: unknown\n"));
    assert!(lines(Some(&history(None, None))).ends_with("Queue position: not queued\n"));
}

#[test]
fn the_json_document_gains_only_the_history_key() {
    let v1 = json!({"job_id": "x"});
    assert_eq!(document(v1.clone(), None), v1);
    let extended = document(v1, Some(&history(None, None)));
    assert_eq!(extended["job_id"], "x");
    assert_eq!(extended[KEY]["decisions"][1]["reason"], "start_rate");
    assert_eq!(extended.as_object().unwrap().len(), 2);
}

#[test]
fn an_unadvertised_daemon_is_never_asked() {
    let dir = std::env::temp_dir().join(format!("arc-history-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let socket = dir.join("arc.sock");
    assert!(!advertised(&socket, std::process::id()));
    std::fs::write(dir.join(ADVERTISEMENT), "").unwrap();
    // A file nobody locks advertises nothing.
    assert!(!advertised(&socket, std::process::id()));
    std::fs::remove_dir_all(&dir).unwrap();
}
