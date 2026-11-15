//! Four mixed-language TEST cases share the identical synchronized scenario.
use super::*;

pub fn pair(read: Read, peer: Peer, python_first: bool) {
    let peer = match peer {
        Peer::Cargo => Kind::Cargo,
        Peer::Bun => Kind::Bun,
    };
    let kinds = if python_first {
        [Kind::Pytest, peer]
    } else {
        [peer, Kind::Pytest]
    };
    run_pair(
        read,
        &format!("{:?}-then-{:?}", kinds[0], kinds[1]),
        kinds,
        "class_window",
    );
}
pub fn global_budget(read: Read) {
    // BUILD has a distinct class window. Its default two-core reservation
    // exhausts the existing global budget while the TEST class is still empty.
    run_pair(
        read,
        "global-cpu-budget",
        [Kind::Build, Kind::Pytest],
        "cpu_budget",
    );
}
fn run_pair(read: Read, name: &str, kinds: [Kind; 2], wait: &str) {
    let case = case(name, read, false);
    let p = projects::project(&case, "first", kinds[0], true);
    let q = projects::project(&case, "second", kinds[1], true);
    let mut first = start(&case, &p, &kinds[0].argv());
    let entry = entered(&case, &mut first, kinds[0]);
    let first_id = entry["jobId"].as_str().unwrap();
    assert_eq!(case.active()["permits"], 1);
    if wait == "class_window" {
        // Spare CPU and memory prove this wait is the shared TEST window.
        assert_eq!(case.active()["cpu"], 1000);
        assert_eq!(case.active()["memory"], 1073741824_u64);
    } else {
        assert_eq!(case.active()["cpu"], 2000);
    }
    let mut second = start_with(&case, &q, &kinds[1].argv(), |cmd| {
        cmd.env("TBS_PREDECESSOR", first_id);
    });
    let queue = queued(&case, &mut second, kinds[1], wait);
    let second_id = queue["sqlite"]["id"].as_str().unwrap();
    assert_ne!(first_id, second_id);
    assert!(first.wrapper.child.try_wait().unwrap().is_none());
    assert_eq!(case.active()["permits"], 1);
    assert_eq!(log(&q.join("launches")).lines().count(), 0);
    assert_eq!(log(&q.join("bodies")).lines().count(), 0);
    first.finish(0);
    let first_terminal = terminal(&case, &first, kinds[0], 0);
    let second_entry = entered(&case, &mut second, kinds[1]);
    let first_row = case.row(first_id);
    let second_row = case.row(second_id);
    assert!(
        second_row["granted_at_ms"].as_u64().unwrap() >= first_row["released_at"].as_u64().unwrap()
    );
    assert_eq!(first_row["permit_state"], "RELEASED");
    assert_eq!(second_row["permit_state"], "ACTIVE");
    assert_eq!(log(&q.join("release-witness")).trim(), "RELEASED");
    let identity: ProcessIdentity = serde_json::from_value(entry["child"].clone()).unwrap();
    assert_eq!(probe_live(&identity), IdentityStatus::Missing);
    assert_eq!(case.active()["permits"], 1);
    second.finish(0);
    let second_terminal = terminal(&case, &second, kinds[1], 0);
    match kinds.iter().find(|&&k| k == Kind::Cargo || k == Kind::Bun) {
        Some(Kind::Cargo) => {
            let path = if kinds[0] == Kind::Cargo { &p } else { &q };
            assert!(log(&path.join("stdout")).contains("1 passed"));
            assert!(log(&path.join("stdout")).contains("CARGO_TEST_BODY_OK"));
        }
        Some(Kind::Bun) => {
            let path = if kinds[0] == Kind::Bun { &p } else { &q };
            assert!(log(&path.join("stderr")).contains("1 pass"));
            assert!(log(&path.join("stdout")).contains("BUN_TEST_BODY_OK"));
        }
        _ => (),
    }
    let history = permit_history(&case, 2, 1);
    assert_eq!(history["testPermitHighWater"], 1);
    case.save(name, json!({"productionDaemon":true,"kinds":format!("{kinds:?}"),"firstEntry":entry,"durablyQueuedSecond":queue,"secondStartsBeforeRelease":0,"secondEntry":second_entry,"outcomes":[first_terminal,second_terminal],"starts":[1,1],"bodyCounts":[1,1],"releaseWitness":log(&q.join("release-witness")),"permits":history,"distinctProjectRoots":[p,q],"waitReason":wait}));
    case.done();
}
