use crate::{DoctorReport, Request, Response, ERROR_INTEGRITY_FAILED, OPERATION_DOCTOR};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const ERROR_SCHEMA_INCOMPATIBLE: &str = "SCHEMA_INCOMPATIBLE";
pub const ERROR_SCHEMA_DETAILS_UNAVAILABLE: &str = "SCHEMA_DETAILS_UNAVAILABLE";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaStatus {
    Compatible,
    UpgradeRequired,
    MissingHistory,
    ChecksumMismatch,
    InvalidHistory,
    UnsupportedNewer,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaReport {
    pub status: SchemaStatus,
    pub applied_version: Option<u32>,
    pub supported_version: u32,
    pub history_verified: bool,
}

impl DoctorReport {
    /// Validate modern assessment or the original daemon's basic diagnostics.
    /// Legacy success never establishes verified migration history.
    pub fn validation_error(&self) -> Option<&'static str> {
        if self.integrity_check != "ok" {
            return Some(ERROR_INTEGRITY_FAILED);
        }
        match &self.schema {
            Some(schema)
                if schema.status == SchemaStatus::Compatible
                    && schema.history_verified
                    && schema.supported_version > 0
                    && schema.applied_version == Some(schema.supported_version)
                    && self.schema_migrations == i64::from(schema.supported_version) =>
            {
                None
            }
            Some(_) => Some(ERROR_SCHEMA_INCOMPATIBLE),
            None if self.schema_migrations == 1 => None,
            None => Some(ERROR_SCHEMA_DETAILS_UNAVAILABLE),
        }
    }
}

pub(crate) fn doctor_response(request: &Request, report: DoctorReport) -> Response {
    let code = report.validation_error();
    if code == Some(ERROR_INTEGRITY_FAILED) {
        return Response::error(&request.request_id, ERROR_INTEGRITY_FAILED);
    }
    let mut result = json!({
        "operation": OPERATION_DOCTOR,
        "integrity_check": report.integrity_check,
        "schema_migrations": report.schema_migrations,
        "ipc_owner_only": true,
        "tray_connected": false,
        "vendors_detected": false,
        "command_classification": "not_checked",
        "sweeper": "off",
    });
    if let Some(schema) = report.schema {
        result["schema"] = json!(schema);
    }
    if let Some(code) = code {
        let mut response = Response::error(&request.request_id, code);
        response.result = Some(result);
        response
    } else {
        Response::ok(&request.request_id, result)
    }
}
