use super::{native_support::*, serialization, support::*};
use arc_ipc::{governor::*, Request};
use serde_json::{json, Value};
use std::io::{Read, Write};

fn discard_reply_body(case: &Case, operation: &str, payload: Value) -> u32 {
    let mut request = Request::new("lost-response-body", operation);
    request.payload = payload;
    let bytes = serde_json::to_vec(&request).unwrap();
    let mut socket = case.fixture.connect();
    socket
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .unwrap();
    socket.write_all(&bytes).unwrap();
    let mut header = [0; 4];
    // A response starts only after the production writer returns. Read just
    // framing, then close without consuming/decoding a mutation/launch ACK.
    socket.read_exact(&mut header).unwrap();
    let length = u32::from_be_bytes(header);
    assert!(length > 0 && length <= arc_ipc::MAX_FRAME_BYTES);
    drop(socket);
    length
}

#[test]
fn gv_n07_cross_client_cas_conflict_noop_and_historical_replay_match_authority() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    let socket = case.fixture.socket.clone();
    let start = std::sync::Arc::new(std::sync::Barrier::new(2));
    let cli_start = start.clone();
    let cli = std::thread::spawn(move || {
        cli_start.wait();
        std::process::Command::new(binary("arcctl"))
            .args([
                "daemon",
                "pause",
                "--json",
                "--expected-revision",
                "0",
                "--mutation-id",
                "0-00000000000000000000000000000031",
            ])
            .env(arc_core::ARC_SOCKET_PATH, socket)
            .output()
            .unwrap()
    });
    start.wait();
    let bridge_request = intent(0, false, 0x32);
    let bridge = bridge_set(&case, &bridge_request);
    let cli = cli.join().unwrap();
    let cli_result: Value = serde_json::from_slice(&cli.stdout).unwrap();
    let results = [cli_result.clone(), bridge.clone()];
    assert_eq!(
        results
            .iter()
            .filter(|r| r["status"] == "COMMITTED")
            .count(),
        1
    );
    assert_eq!(
        results.iter().filter(|r| r["status"] == "CONFLICT").count(),
        1
    );
    let winner = results.iter().find(|r| r["status"] == "COMMITTED").unwrap();
    let original_request = if winner["mutation_id"] == bridge_request["mutation_id"] {
        bridge_request
    } else {
        intent(0, false, 0x31)
    };
    let first = switches(
        &case,
        false,
        1,
        Some(winner["mutation_id"].as_str().unwrap()),
    );
    let noop = bridge_set(&case, &intent(1, false, 0x33));
    assert_eq!(noop["changed"], false);
    assert_eq!(
        noop["switch"], winner["switch"],
        "no-op must preserve timestamp and revision"
    );
    let mut conflict = original_request.clone();
    conflict["enabled"] = json!(true);
    let conflict = bridge_set(&case, &conflict);
    assert_eq!(conflict["reason"], "MUTATION_ID_CONFLICT");
    let resume = cli_set(&case, &intent(1, true, 0x34));
    let historical = bridge_set(&case, &original_request);
    assert_eq!(historical["replayed"], true);
    assert_eq!(historical["switch"], winner["switch"]);
    let current = switches(
        &case,
        true,
        2,
        Some(winner["mutation_id"].as_str().unwrap()),
    );
    assert_eq!(
        current["registeredBridge"]["data"]["mutation_result"],
        historical
    );
    let receipts=case.rows("SELECT mutation_id,expected_revision,result_revision,changed FROM governor_mutation_receipts ORDER BY sequence");
    assert_eq!(receipts.len(), 3);
    save_case(
        "GV-N07-cross-client",
        json!({"concurrentReplies":results,"cliExit":cli.status.code(),"first":first,
        "noop":noop,"conflict":conflict,"resume":resume,"historical":historical,"current":current,"receipts":receipts}),
    );
    case.stop();
}

#[test]
fn gv_n12_lost_committed_mutation_body_reconnect_and_restart_read_original_receipt() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    let request = intent(0, false, 0x41);
    let length = discard_reply_body(&case, OPERATION_PAUSE, request.clone());
    let read = switches(
        &case,
        false,
        1,
        Some(request["mutation_id"].as_str().unwrap()),
    );
    assert_eq!(
        read["registeredBridge"]["data"]["mutation_result"]["replayed"],
        true
    );
    let receipts = case.rows("SELECT mutation_id,result_revision FROM governor_mutation_receipts");
    assert_eq!(receipts.len(), 1);
    let restarted = restart(&mut case);
    let after = switches(
        &case,
        false,
        1,
        Some(request["mutation_id"].as_str().unwrap()),
    );
    assert_eq!(
        read["registeredBridge"]["data"],
        after["registeredBridge"]["data"]
    );
    let duplicate = cli_set(&case, &request);
    assert_eq!(duplicate["mutation_result"]["replayed"], true);
    assert_eq!(
        case.rows("SELECT mutation_id,result_revision FROM governor_mutation_receipts"),
        receipts
    );
    save_case(
        "GV-N12-lost-mutation",
        json!({"request":request,"discardedBodyLength":length,"readback":read,
        "restart":restarted,"afterRestart":after,"explicitDuplicate":duplicate,"receipts":receipts,"setterCalls":2,"stateTransitions":1}),
    );
    case.stop();
}

#[test]
fn gv_n12_lost_launch_body_never_releases_again_and_legacy_protocol_remains_strict() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    cli_set(&case, &intent(0, false, 0x51));
    let observer =
        arc_core::process_identity::ProcessIdentity::capture(std::process::id()).unwrap();
    let id = arc_core::process_identity::random_job_id().unwrap();
    let classification = json!({"classification":"BUILD","locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"});
    let enqueued=case.rpc(OPERATION_JOB,json!({"action":"TRAFFIC","request":{"action":"ENQUEUE","job_id":id,"observer":observer,"classification":classification}}));
    assert_eq!(enqueued["state"], "UNRESERVED");
    let launch = json!({"mode":"UNRESERVED","job_id":id,"observer":observer});
    let length = discard_reply_body(&case, OPERATION_LAUNCH, launch.clone());
    let retry = case.rpc(OPERATION_LAUNCH, launch);
    assert_eq!(retry["state"], "UNCONFIRMED");
    let claims = case.rows("SELECT job_id,release_id,release_claimed_at_ms FROM governor_jobs");
    assert_eq!(claims.len(), 1);
    assert!(claims[0]["release_id"].as_str().is_some());
    assert!(case
        .rows("SELECT job_id FROM job_lifecycle WHERE child_pid IS NOT NULL")
        .is_empty());
    assert!(case.rows("SELECT job_id FROM permits").is_empty());
    case.rpc(OPERATION_JOB,json!({"action":"TRAFFIC","request":{"action":"CANCEL","job_id":id,"observer":observer,"permit_generation":null}}));
    let legacy_id = arc_core::process_identity::random_job_id().unwrap();
    let legacy=case.rpc("traffic.job",json!({"action":"ENQUEUE","job_id":legacy_id,"observer":observer,"classification":classification}));
    assert_eq!(legacy["state"], "BYPASSED");
    assert_eq!(legacy["reason"], "UNSUPPORTED");
    assert_eq!(
        case.rpc("traffic.capabilities", Value::Null),
        json!(arc_ipc::traffic::Capabilities::supported())
    );
    assert_eq!(
        case.rpc("policy.capabilities", Value::Null),
        json!(arc_ipc::policy::Capabilities::supported())
    );
    assert_eq!(
        case.rows("SELECT job_id FROM governor_jobs").len(),
        1,
        "legacy calls must not be silently opted into a new contract"
    );
    let revision = switches(&case, false, 1, None);
    save_case(
        "GV-N12-lost-launch",
        json!({"enqueued":enqueued,"discardedBodyLength":length,"retry":retry,
        "claims":claims,"actualChildren":0,"permits":[],"legacyOffResult":legacy,"switch":revision}),
    );
    case.stop();
}
