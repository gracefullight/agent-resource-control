use arc_ipc::*;
use serde::{ser::Error as _, Serialize, Serializer};
use serde_json::json;
use std::{
    error::Error,
    io::{self, Cursor, Read, Write},
};

struct FragmentedReader {
    bytes: Cursor<Vec<u8>>,
    interrupt: bool,
    fail_at: Option<u64>,
}
impl Read for FragmentedReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if std::mem::take(&mut self.interrupt) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self.fail_at == Some(self.bytes.position()) {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "read fault"));
        }
        let count = output.len().min(1);
        self.bytes.read(&mut output[..count])
    }
}

fn frame(body: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_frame(&mut bytes, body).unwrap();
    bytes
}

#[test]
fn exchange_writes_one_request_and_reads_one_correlated_response() {
    struct Stream {
        incoming: Cursor<Vec<u8>>,
        outgoing: Vec<u8>,
    }
    impl Read for Stream {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.incoming.read(bytes)
        }
    }
    impl Write for Stream {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.outgoing.write(bytes)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let request = Request::new("exchange-1", OPERATION_HELLO);
    let response = Response::ok("exchange-1", json!({"daemon": "running"}));
    let mut incoming = Vec::new();
    write_response(&mut incoming, &response).unwrap();
    let mut stream = Stream {
        incoming: Cursor::new(incoming),
        outgoing: Vec::new(),
    };
    assert_eq!(exchange(&mut stream, &request).unwrap(), response);
    assert_eq!(
        read_message::<_, Request>(&mut Cursor::new(&stream.outgoing)).unwrap(),
        request
    );
    let outgoing = stream.outgoing.len();
    assert!(matches!(
        exchange(&mut stream, &request),
        Err(IpcError::Frame(FrameError::Closed))
    ));
    assert_eq!(stream.outgoing.len(), outgoing * 2);
}

#[test]
fn fragmented_and_interrupted_reads_recover_without_consuming_the_next_frame() {
    let mut bytes = frame(b"{}");
    bytes.extend(frame(b"{\"next\":true}"));
    let mut reader = FragmentedReader {
        bytes: Cursor::new(bytes),
        interrupt: true,
        fail_at: None,
    };
    assert_eq!(read_frame(&mut reader).unwrap(), b"{}");
    assert_eq!(read_frame(&mut reader).unwrap(), b"{\"next\":true}");
    assert!(matches!(read_frame(&mut reader), Err(FrameError::Closed)));
}

#[test]
fn closed_truncated_and_io_failures_remain_distinct() {
    assert!(matches!(
        read_frame(&mut Cursor::new([])),
        Err(FrameError::Closed)
    ));
    for bytes in [vec![0], vec![0, 0, 0], vec![0, 0, 0, 2, b'{']] {
        assert!(matches!(
            read_frame(&mut Cursor::new(bytes)),
            Err(FrameError::UnexpectedEof)
        ));
    }
    for position in [0, 2, 4, 5] {
        let mut reader = FragmentedReader {
            bytes: Cursor::new(frame(b"{}")),
            interrupt: false,
            fail_at: Some(position),
        };
        let error = read_frame(&mut reader).unwrap_err();
        match &error {
            FrameError::Io(cause) => assert_eq!(cause.kind(), io::ErrorKind::BrokenPipe),
            other => panic!("expected IO failure, got {other:?}"),
        }
        assert_eq!(error.to_string(), "read fault");
        assert_eq!(error.source().unwrap().to_string(), "read fault");
        assert_eq!(reader.bytes.position(), position);
    }
}

struct FaultWriter {
    bytes: Vec<u8>,
    fail_at: Option<usize>,
    fail_flush: bool,
}
impl Write for FaultWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_at == Some(self.bytes.len()) {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "write fault"));
        }
        let count = bytes.len().min(1);
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "flush fault",
            ));
        }
        Ok(())
    }
}

#[test]
fn header_body_and_flush_failures_preserve_the_original_io_error() {
    let expected = frame(b"{}");
    for (fail_at, fail_flush, emitted, kind) in [
        (Some(0), false, 0, io::ErrorKind::BrokenPipe),
        (Some(2), false, 2, io::ErrorKind::BrokenPipe),
        (Some(4), false, 4, io::ErrorKind::BrokenPipe),
        (Some(5), false, 5, io::ErrorKind::BrokenPipe),
        (None, true, 6, io::ErrorKind::ConnectionReset),
    ] {
        let mut writer = FaultWriter {
            bytes: Vec::new(),
            fail_at,
            fail_flush,
        };
        match write_frame(&mut writer, b"{}").unwrap_err() {
            FrameError::Io(error) => assert_eq!(error.kind(), kind),
            other => panic!("expected IO failure, got {other:?}"),
        }
        assert_eq!(writer.bytes, expected[..emitted]);
    }
}

#[test]
fn outbound_frame_boundaries_reject_before_writing_and_accept_the_exact_limit() {
    let mut output = Vec::new();
    assert!(matches!(
        write_frame(&mut output, b""),
        Err(FrameError::EmptyFrame)
    ));
    assert!(output.is_empty());
    let oversized = vec![b'x'; MAX_FRAME_BYTES as usize + 1];
    assert!(
        matches!(write_frame(&mut output, &oversized), Err(FrameError::TooLarge { declared }) if declared == MAX_FRAME_BYTES + 1)
    );
    assert!(output.is_empty());
    let body = &oversized[..MAX_FRAME_BYTES as usize];
    write_frame(&mut output, body).unwrap();
    assert_eq!(&output[..4], &MAX_FRAME_BYTES.to_be_bytes());
    assert_eq!(read_frame(&mut Cursor::new(output)).unwrap(), body);
}

#[test]
fn frame_error_messages_and_sources_describe_each_failure() {
    for (error, expected) in [
        (FrameError::Closed, "connection closed".to_owned()),
        (FrameError::UnexpectedEof, "truncated frame".to_owned()),
        (FrameError::EmptyFrame, "empty frame".to_owned()),
        (
            FrameError::TooLarge {
                declared: MAX_FRAME_BYTES + 1,
            },
            format!(
                "declared frame length {} exceeds {}",
                MAX_FRAME_BYTES + 1,
                MAX_FRAME_BYTES
            ),
        ),
    ] {
        assert_eq!(error.to_string(), expected);
        assert!(error.source().is_none());
    }
    let error = FrameError::from(io::Error::new(io::ErrorKind::TimedOut, "frame timeout"));
    assert_eq!(error.to_string(), "frame timeout");
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        io::ErrorKind::TimedOut
    );
}

#[test]
fn ipc_errors_keep_display_and_source_chains() {
    let wrapped = IpcError::from(FrameError::UnexpectedEof);
    assert_eq!(wrapped.to_string(), "truncated frame");
    assert!(wrapped.source().unwrap().is::<FrameError>());
    let io = IpcError::from(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));
    assert_eq!(io.to_string(), "denied");
    assert_eq!(
        io.source()
            .unwrap()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    let json = serde_json::from_slice::<serde_json::Value>(b"{").unwrap_err();
    let expected = json.to_string();
    let json = IpcError::from(json);
    assert_eq!(json.to_string(), expected);
    assert!(json.source().unwrap().is::<serde_json::Error>());
    assert_eq!(
        IpcError::NotAnObject.to_string(),
        "frame is not a JSON object"
    );
    assert!(IpcError::NotAnObject.source().is_none());
}

struct SerializationFailure;
impl Serialize for SerializationFailure {
    fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(S::Error::custom("injected serialization failure"))
    }
}

#[test]
fn failed_serialization_emits_nothing_and_framed_decode_keeps_type_errors() {
    let mut output = Vec::new();
    let error = write_message(&mut output, &SerializationFailure).unwrap_err();
    assert!(matches!(error, IpcError::Json(_)));
    assert_eq!(error.to_string(), "injected serialization failure");
    assert!(output.is_empty());
    let error = read_message::<_, Request>(&mut Cursor::new(frame(b"{}"))).unwrap_err();
    assert!(matches!(error, IpcError::Json(_)));
    let error = read_message::<_, Request>(&mut Cursor::new(frame(b"{"))).unwrap_err();
    assert!(matches!(error, IpcError::Json(_)));
}

#[test]
fn failing_doctor_returns_only_integrity_error_with_request_correlation() {
    let request = Request::new("doctor-fault", OPERATION_DOCTOR);
    let response = dispatch(&request, || Err("internal diagnostic must not leak".into()));
    assert_eq!(
        response,
        Response::error("doctor-fault", ERROR_INTEGRITY_FAILED)
    );
    let mut output = Vec::new();
    write_response(&mut output, &response).unwrap();
    let decoded = read_response(&mut Cursor::new(output)).unwrap();
    assert_eq!(decoded, response);
    assert!(!serde_json::to_string(&decoded)
        .unwrap()
        .contains("internal diagnostic"));
    assert_eq!(
        dispatch(&Request::new("next", OPERATION_HELLO), || unreachable!()).status,
        STATUS_OK
    );
    assert_eq!(
        serde_json::to_value(decoded).unwrap()["request_id"],
        json!("doctor-fault")
    );
}
