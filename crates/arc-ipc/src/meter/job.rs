//! Additive protocol-1 helpers. Daemon handlers must still authenticate the peer
//! and bind the report to durable lifecycle facts before accepting it.
use crate::{Request, Response, ERROR_INVALID_REQUEST, PROTOCOL_VERSION, STATUS_ERROR, STATUS_OK};
pub use arc_core::job_meter::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

pub const OPERATION_METER_REPORT: &str = "meter.report";
pub const OPERATION_METER_SUMMARY: &str = "meter.summary";
pub const ERROR_METER_REPORT_CONFLICT: &str = "METER_REPORT_CONFLICT";
pub const ERROR_METER_IDENTITY_MISMATCH: &str = "METER_IDENTITY_MISMATCH";
pub const ERROR_METER_NOT_TERMINAL: &str = "METER_NOT_TERMINAL";
pub const METER_REPORT_MAX_BUDGET_MS: u64 = 500;
pub const METER_SUMMARY_MAX_BUDGET_MS: u64 = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryRequest {
    pub job_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportDisposition {
    Accepted,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportReceipt {
    pub schema_version: u32,
    pub job_id: String,
    pub report_id: String,
    pub disposition: ReportDisposition,
}

// Only missing_coverage has an array in this schema; its items are enums encoded
// as strings. Reject serde's positional struct-array alternative everywhere.
fn object_tree(value: &Value) -> bool {
    match value {
        Value::Array(items) => items.iter().all(Value::is_string),
        Value::Object(map) => map.values().all(object_tree),
        _ => true,
    }
}
fn decode<T: DeserializeOwned>(value: &Value) -> Result<T, &'static str> {
    if !value.is_object()
        || !object_tree(value)
        || serde_json::to_vec(value).map_or(true, |bytes| bytes.len() > MAX_JOB_METER_BYTES)
    {
        return Err(ERROR_INVALID_REQUEST);
    }
    T::deserialize(value).map_err(|_| ERROR_INVALID_REQUEST)
}
pub fn decode_report(value: &Value) -> Result<JobMeterReport, &'static str> {
    let report: JobMeterReport = decode(value)?;
    report
        .is_valid()
        .then_some(report)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn decode_summary_request(value: &Value) -> Result<SummaryRequest, &'static str> {
    let request: SummaryRequest = decode(value)?;
    valid_id(&request.job_id)
        .then_some(request)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn decode_summary(value: &Value, job_id: &str) -> Result<JobMeterSummary, &'static str> {
    let summary: JobMeterSummary = decode(value)?;
    (summary.is_valid() && summary.job_id == job_id)
        .then_some(summary)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn decode_report_receipt(
    value: &Value,
    report: &JobMeterReport,
) -> Result<ReportReceipt, &'static str> {
    let receipt: ReportReceipt = decode(value)?;
    (receipt.schema_version == JOB_METER_SCHEMA_VERSION
        && receipt.job_id == report.job_id
        && receipt.report_id == report.report_id)
        .then_some(receipt)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn report_request(
    request_id: impl Into<String>,
    report: &JobMeterReport,
) -> Result<Request, &'static str> {
    // serde_json serializes nonfinite floats as null; validate typed numbers
    // before that lossy conversion can turn invalid input into an unknown.
    if !report.is_valid() {
        return Err(ERROR_INVALID_REQUEST);
    }
    let value = serde_json::to_value(report).map_err(|_| ERROR_INVALID_REQUEST)?;
    decode_report(&value)?;
    let mut request = Request::new(request_id, OPERATION_METER_REPORT);
    request.payload = value;
    Ok(request)
}
pub fn summary_request(
    request_id: impl Into<String>,
    job_id: impl Into<String>,
) -> Result<Request, &'static str> {
    let job_id = job_id.into();
    if !valid_id(&job_id) {
        return Err(ERROR_INVALID_REQUEST);
    }
    let mut request = Request::new(request_id, OPERATION_METER_SUMMARY);
    request.payload = serde_json::json!({ "job_id": job_id });
    Ok(request)
}

#[derive(Debug)]
pub enum JobMeterRequest {
    Report(Box<JobMeterReport>),
    Summary(SummaryRequest),
}
impl JobMeterRequest {
    pub fn decode(operation: &str, payload: &Value) -> Option<Result<Self, &'static str>> {
        Some(match operation {
            OPERATION_METER_REPORT => decode_report(payload).map(|r| Self::Report(Box::new(r))),
            OPERATION_METER_SUMMARY => decode_summary_request(payload).map(Self::Summary),
            _ => return None,
        })
    }
}

pub fn matches_terminal_outcome(report: &JobMeterReport, outcome: &crate::jobs::Outcome) -> bool {
    match (&report.outcome, outcome) {
        (TerminalOutcome::Exited { code: a }, crate::jobs::Outcome::Exited { code: b }) => a == b,
        (
            TerminalOutcome::Signaled {
                signal: a,
                core_dumped: ca,
            },
            crate::jobs::Outcome::Signaled {
                signal: b,
                core_dumped: cb,
            },
        ) => a == b && ca == cb,
        _ => false,
    }
}

fn validate_request(request: &Request, operation: &str) -> Result<(), &'static str> {
    if request.protocol_version != PROTOCOL_VERSION {
        return Err(crate::ERROR_PROTOCOL_INCOMPATIBLE);
    }
    if request.operation != operation {
        return Err(ERROR_INVALID_REQUEST);
    }
    Ok(())
}
pub fn summary_response(request: &Request, summary: &JobMeterSummary) -> Response {
    let result: Result<Value, &'static str> = (|| {
        validate_request(request, OPERATION_METER_SUMMARY)?;
        let query = decode_summary_request(&request.payload)?;
        if !summary.is_valid() {
            return Err(ERROR_INVALID_REQUEST);
        }
        let value = serde_json::to_value(summary).map_err(|_| ERROR_INVALID_REQUEST)?;
        decode_summary(&value, &query.job_id)?;
        Ok(value)
    })();
    match result {
        Ok(value) => Response::ok(&request.request_id, value),
        Err(code) => Response::error(&request.request_id, code),
    }
}
pub fn report_response(request: &Request, receipt: &ReportReceipt) -> Response {
    let result: Result<Value, &'static str> = (|| {
        validate_request(request, OPERATION_METER_REPORT)?;
        let report = decode_report(&request.payload)?;
        let value = serde_json::to_value(receipt).map_err(|_| ERROR_INVALID_REQUEST)?;
        decode_report_receipt(&value, &report)?;
        Ok(value)
    })();
    match result {
        Ok(value) => Response::ok(&request.request_id, value),
        Err(code) => Response::error(&request.request_id, code),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobMeterError {
    Unsupported,
    Unavailable,
    Conflict,
    IdentityMismatch,
    NotTerminal,
    Protocol,
    InvalidReply,
}
fn response_value<'a>(
    request: &Request,
    response: &'a Response,
    operation: &str,
) -> Result<&'a Value, JobMeterError> {
    if request.protocol_version != PROTOCOL_VERSION || response.protocol_version != PROTOCOL_VERSION
    {
        return Err(JobMeterError::Protocol);
    }
    if request.operation != operation || request.request_id != response.request_id {
        return Err(JobMeterError::InvalidReply);
    }
    match (response.status.as_str(), &response.result, &response.error) {
        (STATUS_OK, Some(value), None) => Ok(value),
        (STATUS_ERROR, None, Some(error)) => Err(match error.code.as_str() {
            crate::ERROR_UNSUPPORTED_CAPABILITY => JobMeterError::Unsupported,
            super::ERROR_METER_UNAVAILABLE => JobMeterError::Unavailable,
            ERROR_METER_REPORT_CONFLICT => JobMeterError::Conflict,
            ERROR_METER_IDENTITY_MISMATCH => JobMeterError::IdentityMismatch,
            ERROR_METER_NOT_TERMINAL => JobMeterError::NotTerminal,
            crate::ERROR_PROTOCOL_INCOMPATIBLE => JobMeterError::Protocol,
            _ => JobMeterError::InvalidReply,
        }),
        _ => Err(JobMeterError::InvalidReply),
    }
}
pub fn interpret_summary(
    request: &Request,
    response: &Response,
) -> Result<JobMeterSummary, JobMeterError> {
    let query =
        decode_summary_request(&request.payload).map_err(|_| JobMeterError::InvalidReply)?;
    let value = response_value(request, response, OPERATION_METER_SUMMARY)?;
    decode_summary(value, &query.job_id).map_err(|_| JobMeterError::InvalidReply)
}
pub fn interpret_report(
    request: &Request,
    response: &Response,
) -> Result<ReportReceipt, JobMeterError> {
    let report = decode_report(&request.payload).map_err(|_| JobMeterError::InvalidReply)?;
    let value = response_value(request, response, OPERATION_METER_REPORT)?;
    decode_report_receipt(value, &report).map_err(|_| JobMeterError::InvalidReply)
}

/// Existing owner-authenticated transport; one clamped absolute deadline across
/// all retries. No execution, daemon launch or background uploader is created.
#[cfg(unix)]
pub fn report_until(
    socket: &std::path::Path,
    request_id: impl Into<String>,
    report: &JobMeterReport,
    deadline: std::time::Instant,
    cancelled: &dyn Fn() -> bool,
    possibly_committed: &mut bool,
) -> Result<ReportReceipt, crate::client::Failure> {
    use crate::client::{exchange_until, ExchangeOptions, Failure};
    let request = report_request(request_id, report).map_err(|_| Failure::Local)?;
    let deadline = deadline.min(
        std::time::Instant::now() + std::time::Duration::from_millis(METER_REPORT_MAX_BUDGET_MS),
    );
    let value = exchange_until(
        socket,
        &request,
        deadline,
        cancelled,
        possibly_committed,
        ExchangeOptions {
            continuity: None,
            retry_busy: true,
        },
    )?;
    decode_report_receipt(&value, report).map_err(|_| {
        *possibly_committed = true;
        Failure::InvalidReply
    })
}
#[cfg(unix)]
pub fn summary_until(
    socket: &std::path::Path,
    request_id: impl Into<String>,
    job_id: &str,
    deadline: std::time::Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<JobMeterSummary, crate::client::Failure> {
    use crate::client::{exchange_until, ExchangeOptions, Failure};
    let request = summary_request(request_id, job_id).map_err(|_| Failure::Local)?;
    let deadline = deadline.min(
        std::time::Instant::now() + std::time::Duration::from_millis(METER_SUMMARY_MAX_BUDGET_MS),
    );
    let value = exchange_until(
        socket,
        &request,
        deadline,
        cancelled,
        &mut false,
        ExchangeOptions {
            continuity: None,
            retry_busy: false,
        },
    )?;
    decode_summary(&value, job_id).map_err(|_| Failure::InvalidReply)
}

#[cfg(test)]
mod tests;
