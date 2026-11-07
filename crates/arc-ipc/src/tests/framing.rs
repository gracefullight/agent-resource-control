use super::*;
use std::io::Cursor;

#[test]
fn frame_round_trips_one_json_object() {
    let request = hello();
    let mut buffer = Vec::new();
    write_message(&mut buffer, &request).unwrap();
    assert_eq!(
        u32::from_be_bytes(buffer[..4].try_into().unwrap()) as usize,
        buffer.len() - 4
    );
    let mut reader = Cursor::new(buffer);
    let decoded: Request = read_message(&mut reader).unwrap();
    assert_eq!(decoded, request);
}

#[test]
fn writing_non_object_messages_is_rejected_before_emitting_a_frame() {
    for message in [
        json!([1, "req-1", "daemon.hello"]),
        json!("text"),
        json!(42),
        json!(null),
        json!(true),
    ] {
        let mut output = Vec::new();
        assert!(matches!(
            write_message(&mut output, &message),
            Err(IpcError::NotAnObject)
        ));
        assert!(output.is_empty());
    }
}

#[test]
fn non_object_roots_are_rejected() {
    let bodies: [&[u8]; 6] = [
        br#"[1,"req-1","daemon.hello",null]"#,
        br#"[1,"req-1","daemon.hello"]"#,
        br#""daemon.hello""#,
        b"42",
        b"null",
        b"true",
    ];
    for body in bodies {
        assert!(
            matches!(Request::decode(body), Err(IpcError::NotAnObject)),
            "{} must not decode as a request",
            String::from_utf8_lossy(body)
        );
    }
}

#[test]
fn multiple_json_values_are_rejected() {
    let body = br#"{"protocol_version":1,"request_id":"a","operation":"daemon.hello"} {"protocol_version":1,"request_id":"b","operation":"daemon.hello"}"#;
    assert!(matches!(Request::decode(body), Err(IpcError::Json(_))));
}

#[test]
fn a_framed_array_is_rejected_without_reaching_a_handler() {
    let mut frame = Vec::new();
    write_frame(&mut frame, br#"[1,"req-1","daemon.hello",null]"#).unwrap();
    let mut reader = Cursor::new(frame);
    assert!(matches!(
        read_message::<_, Request>(&mut reader),
        Err(IpcError::NotAnObject)
    ));
}

#[test]
fn responses_are_objects_too() {
    let mut frame = Vec::new();
    write_frame(&mut frame, br#"[1,"req-1","ok",null,null]"#).unwrap();
    assert!(matches!(
        read_response(&mut Cursor::new(frame)),
        Err(IpcError::NotAnObject)
    ));

    let response = Response::ok("req-2", json!({"daemon": "running"}));
    let mut buffer = Vec::new();
    write_response(&mut buffer, &response).unwrap();
    assert_eq!(read_response(&mut Cursor::new(buffer)).unwrap(), response);
}

#[test]
fn an_object_request_still_decodes_after_a_rejected_array() {
    assert!(matches!(
        Request::decode(br#"[1,"req-1","daemon.hello",null]"#),
        Err(IpcError::NotAnObject)
    ));
    let decoded = Request::decode(
        br#"{"protocol_version":1,"request_id":"req-2","operation":"daemon.hello"}"#,
    )
    .unwrap();
    assert_eq!(decoded, Request::new("req-2", OPERATION_HELLO));
}

#[test]
fn oversize_declared_length_is_rejected_without_reading_the_body() {
    let mut frame = (MAX_FRAME_BYTES + 1).to_be_bytes().to_vec();
    frame.extend_from_slice(b"body that must not be read");
    let mut reader = Cursor::new(frame);
    match read_frame(&mut reader) {
        Err(FrameError::TooLarge { declared }) => {
            assert_eq!(declared, MAX_FRAME_BYTES + 1)
        }
        other => panic!("expected TooLarge, got {other:?}"),
    }
    assert_eq!(reader.position(), 4);
}

#[test]
fn zero_length_is_rejected_without_reading_a_body() {
    let mut reader = Cursor::new(vec![0, 0, 0, 0]);
    assert!(matches!(
        read_frame(&mut reader),
        Err(FrameError::EmptyFrame)
    ));
    assert_eq!(reader.position(), 4);
}
