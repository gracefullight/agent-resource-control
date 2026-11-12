//! Owner-only local IPC framing for ARC.
//!
//! A frame is a u32 big-endian length prefix followed by exactly one UTF-8
//! JSON object. The protocol version is 1 and a frame may not exceed 1 MiB.
//! Classification command text is transient and never sent to storage or logs.

#[cfg(unix)]
pub mod client;
pub mod jobs;
pub mod meter;
pub mod policy;
mod schema;
pub mod traffic;
pub use schema::{
    SchemaReport, SchemaStatus, ERROR_SCHEMA_DETAILS_UNAVAILABLE, ERROR_SCHEMA_INCOMPATIBLE,
};

pub use arc_detector::{Classification, ClassifyRequest, JobClass, Locality, Reason, Uncertainty};

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
pub const OPERATION_JOB_CLASSIFY: &str = "job.classify";
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
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub protocol_version: u32,
    pub request_id: String,
    pub operation: String,
    #[serde(default)]
    pub payload: Value,
}

impl fmt::Debug for Request {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Request")
            .field("request_id", &self.request_id)
            .field("operation", &self.operation)
            .field("payload", &Redacted)
            .finish()
    }
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
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub protocol_version: u32,
    pub request_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

impl fmt::Debug for Response {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Response")
            .field("request_id", &self.request_id)
            .field("status", &self.status)
            .field("result", &Redacted)
            .finish()
    }
}

/// Never inspect or format an envelope's unvalidated JSON value for Debug.
struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<SchemaReport>,
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
        OPERATION_JOB_CLASSIFY => classify_response(request),
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
            Ok(report) => schema::doctor_response(request, report),
            Err(_) => Response::error(&request.request_id, ERROR_INTEGRITY_FAILED),
        },
        _ => Response::error(&request.request_id, ERROR_UNSUPPORTED_CAPABILITY),
    }
}

fn classify_response(request: &Request) -> Response {
    if !request.payload.is_object() {
        return Response::error(&request.request_id, ERROR_INVALID_REQUEST);
    }
    // Borrow the payload while decoding; never clone its raw command tree.
    let Ok(input) = ClassifyRequest::deserialize(&request.payload) else {
        return Response::error(&request.request_id, ERROR_INVALID_REQUEST);
    };
    let result = arc_detector::classify(&input);
    Response::ok(&request.request_id, json!(result))
}

#[cfg(test)]
mod tests;
