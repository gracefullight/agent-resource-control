//! Owner-only local IPC framing for ARC.
//!
//! A frame is a u32 big-endian length prefix followed by exactly one UTF-8
//! JSON object. The protocol version is 1 and a frame may not exceed 1 MiB.
//! Messages never carry credentials, chat text, or cookies.

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    error::Error,
    fmt,
    io::{self, Read, Write},
};

/// The only protocol version this slice accepts.
pub const PROTOCOL_VERSION: u32 = 1;
/// Maximum accepted frame body, including the JSON object only.
pub const MAX_FRAME_BYTES: u32 = 1024 * 1024;

pub const OPERATION_HELLO: &str = "daemon.hello";
pub const OPERATION_HEALTH: &str = "daemon.health";
pub const OPERATION_DOCTOR: &str = "diagnostics.doctor";

pub const STATUS_OK: &str = "ok";
pub const STATUS_ERROR: &str = "error";

pub const ERROR_PROTOCOL_INCOMPATIBLE: &str = "PROTOCOL_INCOMPATIBLE";
pub const ERROR_UNSUPPORTED_CAPABILITY: &str = "UNSUPPORTED_CAPABILITY";
pub const ERROR_INVALID_REQUEST: &str = "INVALID_REQUEST";
pub const ERROR_FRAME_TOO_LARGE: &str = "FRAME_TOO_LARGE";
pub const ERROR_INTEGRITY_FAILED: &str = "INTEGRITY_FAILED";
/// Diagnostics could not be admitted or completed within the database budget.
pub const ERROR_DB_BUSY: &str = "DB_BUSY";

/// One request object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub protocol_version: u32,
    pub request_id: String,
    pub operation: String,
    #[serde(default)]
    pub payload: Value,
}

impl Request {
    /// Build a protocol-1 request with no payload.
    pub fn new(request_id: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            operation: operation.into(),
            payload: Value::Null,
        }
    }

    pub fn decode(frame: &[u8]) -> Result<Self, IpcError> {
        decode_object(frame)
    }
}

/// One response object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub protocol_version: u32,
    pub request_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

impl Response {
    pub fn ok(request_id: impl Into<String>, result: Value) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            status: STATUS_OK.to_owned(),
            result: Some(result),
            error: None,
        }
    }

    pub fn error(request_id: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            status: STATUS_ERROR.to_owned(),
            result: None,
            error: Some(ErrorBody {
                code: code.into(),
                message: None,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Values the daemon reads from the database for `diagnostics.doctor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub integrity_check: String,
    pub schema_migrations: i64,
}

/// Errors from reading or writing one frame.
#[derive(Debug)]
pub enum FrameError {
    /// The peer closed before sending a length prefix.
    Closed,
    /// The peer closed in the middle of the length prefix or body.
    UnexpectedEof,
    /// A declared length of zero is not a JSON object.
    EmptyFrame,
    /// The declared body length is above [`MAX_FRAME_BYTES`].
    TooLarge {
        declared: u32,
    },
    Io(io::Error),
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => formatter.write_str("connection closed"),
            Self::UnexpectedEof => formatter.write_str("truncated frame"),
            Self::EmptyFrame => formatter.write_str("empty frame"),
            Self::TooLarge { declared } => {
                write!(
                    formatter,
                    "declared frame length {declared} exceeds {MAX_FRAME_BYTES}"
                )
            }
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for FrameError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for FrameError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Errors from one request/response exchange.
#[derive(Debug)]
pub enum IpcError {
    Frame(FrameError),
    Json(serde_json::Error),
    /// The frame decoded as JSON but its root was not a JSON object.
    NotAnObject,
    Io(io::Error),
}

impl fmt::Display for IpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::NotAnObject => formatter.write_str("frame is not a JSON object"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for IpcError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Frame(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::NotAnObject => None,
            Self::Io(error) => Some(error),
        }
    }
}

impl From<FrameError> for IpcError {
    fn from(error: FrameError) -> Self {
        Self::Frame(error)
    }
}

impl From<serde_json::Error> for IpcError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<io::Error> for IpcError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Read one length-prefixed frame.
///
/// The declared length is validated before any body allocation, so a length
/// above [`MAX_FRAME_BYTES`] is rejected without storing the body.
pub fn read_frame<R: Read>(reader: &mut R) -> Result<Vec<u8>, FrameError> {
    let mut header = [0u8; 4];
    let mut filled = 0;
    while filled < header.len() {
        match reader.read(&mut header[filled..]) {
            Ok(0) if filled == 0 => return Err(FrameError::Closed),
            Ok(0) => return Err(FrameError::UnexpectedEof),
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(FrameError::Io(error)),
        }
    }

    let declared = u32::from_be_bytes(header);
    if declared == 0 {
        return Err(FrameError::EmptyFrame);
    }
    if declared > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge { declared });
    }

    let mut body = vec![0u8; declared as usize];
    reader.read_exact(&mut body).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            FrameError::UnexpectedEof
        } else {
            FrameError::Io(error)
        }
    })?;
    Ok(body)
}

/// Write one length-prefixed frame.
pub fn write_frame<W: Write>(writer: &mut W, body: &[u8]) -> Result<(), FrameError> {
    let declared =
        u32::try_from(body.len()).map_err(|_| FrameError::TooLarge { declared: u32::MAX })?;
    if declared == 0 {
        return Err(FrameError::EmptyFrame);
    }
    if declared > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge { declared });
    }
    writer.write_all(&declared.to_be_bytes())?;
    writer.write_all(body)?;
    writer.flush()?;
    Ok(())
}

/// Serialize and write one JSON object as a frame.
pub fn write_message<W: Write, T: Serialize>(writer: &mut W, message: &T) -> Result<(), IpcError> {
    let value = serde_json::to_value(message)?;
    if !value.is_object() {
        return Err(IpcError::NotAnObject);
    }
    let body = serde_json::to_vec(&value)?;
    write_frame(writer, &body)?;
    Ok(())
}

/// Read one frame and decode it as JSON.
pub fn read_message<R: Read, T: DeserializeOwned>(reader: &mut R) -> Result<T, IpcError> {
    let frame = read_frame(reader)?;
    decode_object(&frame)
}

/// Decode one frame whose root must be a single JSON object.
///
/// Serde's derived struct deserializer also accepts positional sequences, so
/// the root value is checked before any struct decoding happens. Messages are
/// objects by contract, and a sequence is never a message.
fn decode_object<T: DeserializeOwned>(frame: &[u8]) -> Result<T, IpcError> {
    let value: Value = serde_json::from_slice(frame)?;
    if !value.is_object() {
        return Err(IpcError::NotAnObject);
    }
    Ok(serde_json::from_value(value)?)
}

pub fn write_response<W: Write>(writer: &mut W, response: &Response) -> Result<(), IpcError> {
    write_message(writer, response)
}

pub fn read_response<R: Read>(reader: &mut R) -> Result<Response, IpcError> {
    read_message(reader)
}

/// Send one request and read one response on the same stream.
pub fn exchange<S: Read + Write>(stream: &mut S, request: &Request) -> Result<Response, IpcError> {
    write_message(stream, request)?;
    read_response(stream)
}

/// Dispatch one request. Version mismatch is rejected before any handler runs.
pub fn dispatch<F>(request: &Request, doctor: F) -> Response
where
    F: FnOnce() -> Result<DoctorReport, String>,
{
    if request.protocol_version != PROTOCOL_VERSION {
        return Response::error(&request.request_id, ERROR_PROTOCOL_INCOMPATIBLE);
    }

    match request.operation.as_str() {
        OPERATION_HELLO => Response::ok(
            &request.request_id,
            json!({
                "daemon": "running",
                "protocol_version": PROTOCOL_VERSION,
                "sweeper": "off",
            }),
        ),
        OPERATION_HEALTH => Response::ok(
            &request.request_id,
            json!({
                "daemon": "running",
                "protocol_version": PROTOCOL_VERSION,
                "sweeper": "off",
            }),
        ),
        OPERATION_DOCTOR => match doctor() {
            Ok(report) if report.integrity_check == "ok" && report.schema_migrations == 1 => {
                Response::ok(
                    &request.request_id,
                    json!({
                        "operation": OPERATION_DOCTOR,
                        "integrity_check": report.integrity_check,
                        "schema_migrations": report.schema_migrations,
                        "ipc_owner_only": true,
                        "tray_connected": false,
                        "vendors_detected": false,
                        "command_classification": "not_checked",
                        "sweeper": "off",
                    }),
                )
            }
            Ok(_) => Response::error(&request.request_id, ERROR_INTEGRITY_FAILED),
            Err(_) => Response::error(&request.request_id, ERROR_INTEGRITY_FAILED),
        },
        _ => Response::error(&request.request_id, ERROR_UNSUPPORTED_CAPABILITY),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn hello() -> Request {
        Request::new("req-1", OPERATION_HELLO)
    }

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

    #[test]
    fn protocol_version_99_returns_incompatible_and_a_later_hello_succeeds() {
        let mut incompatible = hello();
        incompatible.protocol_version = 99;
        let response = dispatch(&incompatible, || unreachable!("handler must not run"));
        assert_eq!(response.status, STATUS_ERROR);
        assert_eq!(
            response.error.as_ref().map(|error| error.code.as_str()),
            Some(ERROR_PROTOCOL_INCOMPATIBLE)
        );

        let response = dispatch(&hello(), || unreachable!("handler must not run"));
        assert_eq!(response.status, STATUS_OK);
        assert_eq!(response.protocol_version, PROTOCOL_VERSION);
    }

    #[test]
    fn hello_and_health_do_not_require_the_database() {
        for operation in [OPERATION_HELLO, OPERATION_HEALTH] {
            let request = Request::new("req", operation);
            let response = dispatch(&request, || unreachable!("handler must not run"));
            assert_eq!(response.status, STATUS_OK, "{operation}");
            let result = response.result.unwrap();
            assert_eq!(result["protocol_version"], PROTOCOL_VERSION);
            assert_eq!(result["sweeper"], "off");
        }
    }

    #[test]
    fn doctor_requires_ok_integrity_and_one_migration() {
        let request = Request::new("req", OPERATION_DOCTOR);
        let response = dispatch(&request, || {
            Ok(DoctorReport {
                integrity_check: "ok".to_owned(),
                schema_migrations: 1,
            })
        });
        assert_eq!(response.status, STATUS_OK);
        let result = response.result.unwrap();
        assert_eq!(result["integrity_check"], "ok");
        assert_eq!(result["schema_migrations"], 1);
        assert_eq!(result["ipc_owner_only"], true);
        assert_eq!(result["tray_connected"], false);
        assert_eq!(result["vendors_detected"], false);
        assert_eq!(result["sweeper"], "off");

        let response = dispatch(&request, || {
            Ok(DoctorReport {
                integrity_check: "ok".to_owned(),
                schema_migrations: 0,
            })
        });
        assert_eq!(
            response.error.map(|error| error.code),
            Some(ERROR_INTEGRITY_FAILED.to_owned())
        );
    }

    #[test]
    fn unknown_operation_is_unsupported() {
        let request = Request::new("req", "gc.run");
        let response = dispatch(&request, || unreachable!("handler must not run"));
        assert_eq!(
            response.error.map(|error| error.code),
            Some(ERROR_UNSUPPORTED_CAPABILITY.to_owned())
        );
    }
}
