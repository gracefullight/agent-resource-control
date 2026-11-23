//! Final-review regressions through the built CLI and a real daemon.
use crate::{
    journey::{self, Sandbox},
    runtime::Fixture,
    workspace::{self, cache, classify, exec, member, members, Workspace, CI_A, CI_B},
};
use std::path::Path;

pub fn compound_exec(f: &Fixture, s: &Sandbox, root: &Path, command: &str) -> serde_json::Value {
    assert_eq!(
        exec(f, s, root, command),
        ("UNKNOWN".to_owned(), "UNRESOLVED_COMPOUND".to_owned())
    );
    let db = journey::database(f);
    let job_id: String = db
        .query_row(
            "SELECT id FROM jobs ORDER BY rowid DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let queued: i64 = db
        .query_row(
            "SELECT count(*) FROM traffic_entries WHERE job_id = ?1",
            [&job_id],
            |row| row.get(0),
        )
        .unwrap();
    let permits: i64 = db
        .query_row(
            "SELECT count(*) FROM permits WHERE job_id = ?1",
            [&job_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        (queued, permits),
        (0, 0),
        "the mixed local graph is observed without a single-class reservation"
    );
    let mut request = arc_ipc::Request::new(
        "workspace-compound-read",
        arc_ipc::workspace_compound::OPERATION,
    );
    request.payload = serde_json::json!({"action":"read", "job_id":job_id});
    let response = arc_ipc::exchange(&mut f.connect(), &request).unwrap();
    assert_eq!(response.status, arc_ipc::STATUS_OK, "{response:?}");
    let recorded = response.result.unwrap()["compound"].clone();
    assert_eq!(recorded["kind"], "WORKSPACE_COMPOUND");
    assert_eq!(recorded["locality"], "LOCAL_COMPUTE");
    assert_eq!(
        recorded["members"][0]["classification"]["classification"],
        "TEST"
    );
    assert_eq!(
        recorded["members"][1]["classification"]["classification"],
        "BUILD"
    );
    assert_eq!(
        recorded["members"][0]["classification"]["locality"],
        "HOST_LOCAL"
    );
    assert_eq!(
        recorded["members"][1]["classification"]["locality"],
        "HOST_LOCAL"
    );
    assert_eq!(recorded["admission"]["classification"], "UNKNOWN");
    assert_eq!(recorded["admission"]["uncertainty"], "OBSERVE");
    recorded
}

pub fn cache_overrides(f: &Fixture, s: &Sandbox, command: &str, both: &[(String, String, String)]) {
    let tmp = format!("/tmp/arc-detector-workspace-{}", std::process::id());
    let database = format!("{tmp}/arc.sqlite3");
    for (name, env) in [
        ("memory-cache", [("TMPDIR", s.home.to_str().unwrap())]),
        (
            "database-override-cache",
            [("ARC_DB_PATH", database.as_str())],
        ),
    ] {
        let isolated = Workspace::new(s, name, &[("a", &[CI_A]), ("b", &[CI_B])]);
        let first = classify(f, s, &isolated.root, command, &env);
        assert_eq!(cache(&first), "miss", "{env:?}: {first}");
        assert_eq!(members(&first), both);
        let again = classify(f, s, &isolated.root, command, &env);
        assert_eq!(cache(&again), "hit", "{env:?}: {again}");
        assert_eq!(members(&again), both);
        isolated.member("b", &[("ci", "bun test")]);
        let edited = classify(f, s, &isolated.root, command, &env);
        assert_eq!(cache(&edited), "miss", "{env:?}: {edited}");
        assert_eq!(
            members(&edited),
            [member("a", "ci", "TEST"), member("b", "ci", "TEST")]
        );
        assert_eq!(cache(&classify(f, s, &isolated.root, command, &env)), "hit");
        assert_eq!(
            workspace::exec_with_env(f, s, &isolated.root, command, &env).0,
            "TEST"
        );
    }
    assert!(!Path::new(&tmp).exists());
}
