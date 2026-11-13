// Governor transport cases use the actual production Tauri command registry.
// This socket fixture establishes routing/validation, not SQLite durability.
fn governor_request() -> arc_ipc::governor::SetGovernorSwitchRequest {
    arc_ipc::governor::SetGovernorSwitchRequest {
        key: arc_ipc::governor::GovernorSwitchKey::GovernorEnabled,
        enabled: false,
        expected_revision: 4,
        mutation_id: format!("4-{}", "a".repeat(32)),
    }
}
fn governor_state() -> Value {
    json!({"key":"governor.enabled","enabled":false,"revision":5,"updated_at_ms":100})
}
fn governor_committed() -> Value {
    json!({"status":"COMMITTED","mutation_id":governor_request().mutation_id,"switch":governor_state(),"changed":true,"replayed":false})
}
#[test]
fn registered_governor_commands_use_the_same_rust_dtos_and_authenticated_bridge() {
    use arc_ipc::governor::*;
    let peer = Peer::new(2, |stream, request| {
        assert_eq!(request.protocol_version, 1);
        let result = match request.operation.as_str() {
            OPERATION_SET_SWITCH => {
                assert_eq!(decode_set_switch(&request.operation, &request.payload).unwrap(), governor_request());
                governor_committed()
            }
            OPERATION_GET_SWITCH => {
                let query = decode_get_switch(&request.payload).unwrap();
                assert_eq!(query.mutation_id, Some(governor_request().mutation_id));
                { let mut receipt=governor_committed(); receipt["replayed"]=json!(true); json!({"switch":governor_state(),"mutation_result":receipt}) }
            }
            _ => panic!("unexpected Governor operation"),
        };
        arc_ipc::write_response(stream, &Response::ok(request.request_id, result)).unwrap();
    });
    let app = App::new(peer.bridge());
    assert_eq!(app.invoke("governor_set_switch",json!({"request":governor_request()})).unwrap(),json!({"data":governor_committed(),"received_at_ms":RECEIVED}));
    let reply = app.invoke("governor_get_switch",json!({"query":{"key":"governor.enabled","mutation_id":governor_request().mutation_id}})).unwrap();
    let result = decode_switch_read_result(&reply["data"], &GetGovernorSwitchRequest { mutation_id:Some(governor_request().mutation_id), ..Default::default() }).unwrap();
    assert_eq!(result.switch.revision,5);
    assert!(matches!(result.mutation_result,Some(GovernorMutationResult::Committed { replayed:true,.. })));
}
#[test]
fn registered_governor_validates_inputs_before_dispatch_and_retains_typed_conflicts() {
    use arc_ipc::governor::*;
    let peer=Peer::new(1,|stream,request| {
        assert_eq!(request.operation,OPERATION_SET_SWITCH);
        arc_ipc::write_response(stream,&Response::ok(request.request_id,json!({"status":"CONFLICT","mutation_id":governor_request().mutation_id,"switch":governor_state(),"reason":"REVISION_CONFLICT"}))).unwrap();
    });
    let app=App::new(peer.bridge());
    for payload in [json!({"request": {"key":"other"}}),json!({"request": {"key":"governor.enabled","enabled":false,"expected_revision":4,"mutation_id":"3-invalid"}}),json!({"request": {"key":"governor.enabled","enabled":false,"expected_revision":4,"mutation_id":governor_request().mutation_id,"private":true}})] {
        assert!(app.invoke("governor_set_switch",payload).is_err());
    }
    assert!(app.invoke("governor_get_switch",json!({"query":{"key":"governor.enabled","mutation_id":"invalid"}})).is_err());
    let result=app.invoke("governor_set_switch",json!({"request":governor_request()})).unwrap();
    assert_eq!(result["data"]["status"],"CONFLICT");
    assert_eq!(peer.requests.load(Ordering::Relaxed),1);
}
#[test]
fn registered_governor_busy_lost_reply_and_malformed_ack_are_uncertain_without_write_retries() {
    use arc_ipc::governor::*;
    for kind in ["busy","lost","invalid","explicit_uncertain"] {
        let peer=Peer::new(1,move |stream,request| {
            assert_eq!(request.operation,OPERATION_SET_SWITCH);
            match kind {
                "busy" => arc_ipc::write_response(stream,&Response::error(request.request_id,arc_ipc::ERROR_DB_BUSY)).unwrap(),
                "lost" => (),
                "invalid" => arc_ipc::write_response(stream,&Response::ok(request.request_id,json!({"status":"COMMITTED"}))).unwrap(),
                _ => arc_ipc::write_response(stream,&Response::error(request.request_id,ERROR_WRITE_UNCERTAIN)).unwrap(),
            }
        });
        let app=App::new(peer.bridge());
        assert_eq!(app.invoke("governor_set_switch",json!({"request":governor_request()})),Err(error("uncertain",true)));
        assert_eq!(peer.requests.load(Ordering::Relaxed),1);
    }
}
#[test]
fn governor_bridge_single_flight_owns_dropped_waiter_and_pre_dispatch_rejection() {
    use arc_ipc::governor::*;
    let (sent,received)=mpsc::channel();
    let (release,continue_after)=mpsc::channel();
    let peer=Peer::new(1,move |stream,request| {
        sent.send(()).unwrap();
        continue_after.recv_timeout(Duration::from_secs(3)).unwrap();
        arc_ipc::write_response(stream,&Response::ok(request.request_id,governor_committed())).unwrap();
    });
    let bridge=peer.bridge();
    let worker=bridge.clone();
    let handle=tauri::async_runtime::spawn(async move { worker.governor_set_switch_async(governor_request()).await });
    received.recv_timeout(Duration::from_secs(3)).unwrap();
    // Retiring a JS/read waiter cannot release the blocking task's permit.
    drop(handle);
    assert_eq!(bridge.governor_get_switch(GetGovernorSwitchRequest::default()).unwrap_err().code,ErrorCode::Busy);
    assert_eq!(bridge.governor_set_switch(governor_request()).unwrap_err().code,ErrorCode::Rejected);
    release.send(()).unwrap();
    drop(peer);
}
#[test]
fn governor_bridge_never_forges_commit_confirmation_from_an_unsafe_clock() {
    let peer=Peer::result(governor_committed());
    let bridge=DesktopBridge::with_socket_and_clock(peer.path.clone(),||u64::MAX);
    assert_eq!(bridge.governor_set_switch(governor_request()).unwrap_err().code,ErrorCode::Uncertain);
    bridge.shutdown();
    assert_eq!(bridge.governor_set_switch(governor_request()).unwrap_err().code,ErrorCode::Rejected);
}
