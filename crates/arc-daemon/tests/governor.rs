//! Production startup/socket/auth/writer/restart; no socket mocks or installed
//! daemon. Full synchronized CLI/registered desktop acceptance remains task-8.
#[allow(dead_code)] // Shared native support has broader legacy/load helpers.
#[path = "support/runtime.rs"]
mod runtime;
mod support;
use arc_ipc::{governor::*, Request};
use std::time::{Duration, Instant};
fn intent(base: u64, enabled: bool, n: u32) -> SetGovernorSwitchRequest {
    SetGovernorSwitchRequest {
        key: GovernorSwitchKey::GovernorEnabled,
        enabled,
        expected_revision: base,
        mutation_id: format!("{base}-{n:032x}"),
    }
}
fn rpc(
    f: &runtime::Fixture,
    op: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value, arc_ipc::client::Failure> {
    let mut request = Request::new("native-governor-runtime", op);
    request.payload = payload;
    let mut client = arc_ipc::client::Session::connect_until(
        &f.socket,
        Instant::now() + Duration::from_secs(2),
        &|| false,
        None,
    )
    .unwrap();
    client.exchange(&request, &|| false)
}
fn read(f: &runtime::Fixture, id: Option<String>) -> GovernorSwitchReadResult {
    let q = GetGovernorSwitchRequest {
        key: GovernorSwitchKey::GovernorEnabled,
        mutation_id: id,
    };
    decode_switch_read_result(
        &rpc(f, OPERATION_GET_SWITCH, serde_json::json!(q)).unwrap(),
        &q,
    )
    .unwrap()
}
fn sqlite(f: &runtime::Fixture) -> GovernorSwitchState {
    let c = rusqlite::Connection::open_with_flags(
        &f.database,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    arc_storage::governor::get_switch(&c).unwrap()
}
#[test]
fn production_owner_socket_switch_commit_and_receipt_survive_native_daemon_restart() {
    let _serial = support::serial();
    let f = runtime::Fixture::new();
    let mut daemon = f.spawn("");
    let first_pid = daemon.child.id();
    let caps = rpc(&f, OPERATION_CAPABILITIES, serde_json::Value::Null).unwrap();
    assert_eq!(caps, serde_json::json!(GovernorCapabilities::supported()));
    assert_eq!(read(&f, None).switch, GovernorSwitchState::default());
    let pause = intent(0, false, 1);
    let committed = decode_mutation_result(
        &rpc(&f, OPERATION_PAUSE, serde_json::json!(pause)).unwrap(),
        &pause,
    )
    .unwrap();
    assert!(!committed.switch().enabled);
    assert_eq!(committed.switch().revision, 1);
    assert_eq!(sqlite(&f), *committed.switch());
    assert!(matches!(
        rpc(&f, OPERATION_RESUME, serde_json::json!(pause)),
        Err(arc_ipc::client::Failure::Rejected)
    ));
    assert_eq!(sqlite(&f), *committed.switch());
    daemon.stop(libc::SIGTERM);
    let shutdown_first = f.report();
    assert_eq!(shutdown_first["accounted"], true);
    // Export receipts are create-new. Preserve the first fixture's evidence
    // under another owned name before restarting with a fresh report path.
    std::fs::rename(
        f.root.join("runtime.json"),
        f.root.join("first-runtime.json"),
    )
    .unwrap();
    let mut daemon = f.spawn("");
    let second_pid = daemon.child.id();
    assert_ne!(first_pid, second_pid);
    let restored = read(&f, Some(pause.mutation_id.clone()));
    assert_eq!(restored.switch, *committed.switch());
    assert_eq!(sqlite(&f), restored.switch);
    assert!(matches!(
        restored.mutation_result,
        Some(GovernorMutationResult::Committed { replayed: true, .. })
    ));
    let replay = decode_mutation_result(
        &rpc(&f, OPERATION_SET_SWITCH, serde_json::json!(pause)).unwrap(),
        &pause,
    )
    .unwrap();
    assert!(matches!(
        replay,
        GovernorMutationResult::Committed { replayed: true, .. }
    ));
    let resume = intent(1, true, 2);
    let resumed = decode_mutation_result(
        &rpc(&f, OPERATION_RESUME, serde_json::json!(resume)).unwrap(),
        &resume,
    )
    .unwrap();
    assert!(resumed.switch().enabled);
    assert_eq!(resumed.switch().revision, 2);
    assert_eq!(sqlite(&f), *resumed.switch());
    let noop = intent(2, true, 3);
    let unchanged = decode_mutation_result(
        &rpc(&f, OPERATION_RESUME, serde_json::json!(noop)).unwrap(),
        &noop,
    )
    .unwrap();
    assert!(matches!(
        unchanged,
        GovernorMutationResult::Committed { changed: false, .. }
    ));
    assert_eq!(unchanged.switch(), resumed.switch());
    daemon.stop(libc::SIGTERM);
    let shutdown_second = f.report();
    assert_eq!(shutdown_second["accounted"], true);
    eprintln!(
        "governor-process-observation={}",
        serde_json::json!({"platform":std::env::consts::OS,"daemonPids":[first_pid,second_pid],"default":GovernorSwitchState::default(),"committedPause":committed,"restartReadback":restored.switch,"resumed":resumed,"bothShutdownsAccounted":true})
    );
}
