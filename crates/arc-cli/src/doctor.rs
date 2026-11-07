use arc_ipc::{
    DoctorReport, Response, SchemaReport, SchemaStatus, ERROR_SCHEMA_DETAILS_UNAVAILABLE,
    ERROR_SCHEMA_INCOMPATIBLE, OPERATION_DOCTOR, STATUS_ERROR, STATUS_OK,
};
use std::path::Path;

pub(crate) fn run(socket: &Path) -> Result<(), String> {
    let response = super::request(socket, OPERATION_DOCTOR)?;
    let text = render(response)?;
    println!("{text}");
    Ok(())
}

fn render(response: Response) -> Result<String, String> {
    if response.protocol_version != arc_ipc::PROTOCOL_VERSION {
        return Err(super::PROTOCOL_INCOMPATIBLE.into());
    }
    if response.status == STATUS_ERROR {
        let code = response
            .error
            .as_ref()
            .ok_or(super::IPC_ERROR)?
            .code
            .as_str();
        return Err(match code {
            ERROR_SCHEMA_INCOMPATIBLE => {
                let report = decode(response.result)?;
                match report.schema {
                    Some(schema) => schema_failure(&schema),
                    None => ERROR_SCHEMA_DETAILS_UNAVAILABLE.into(),
                }
            }
            ERROR_SCHEMA_DETAILS_UNAVAILABLE => ERROR_SCHEMA_DETAILS_UNAVAILABLE.into(),
            arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE => super::PROTOCOL_INCOMPATIBLE.into(),
            arc_ipc::ERROR_INTEGRITY_FAILED => super::INTEGRITY_FAILED.into(),
            _ => super::IPC_ERROR.into(),
        });
    }
    if response.status != STATUS_OK || response.error.is_some() {
        return Err(super::IPC_ERROR.into());
    }
    let report = decode(response.result)?;
    if let Some(code) = report.validation_error() {
        return Err(if code == ERROR_SCHEMA_INCOMPATIBLE {
            schema_failure(report.schema.as_ref().expect("present incompatible report"))
        } else {
            code.into()
        });
    }
    let schema_text = match report.schema {
        Some(schema) => format!(
            "Schema compatible. Version {}. Migration history verified.",
            schema.supported_version
        ),
        None => "Schema compatibility details unavailable (legacy daemon).".into(),
    };
    Ok(format!(
        "ok. IPC owner-only. Tray not connected. Vendors not detected. \
         Command classification not checked. Sweeper off. {schema_text}"
    ))
}

fn decode(value: Option<serde_json::Value>) -> Result<DoctorReport, String> {
    serde_json::from_value(value.ok_or(ERROR_SCHEMA_DETAILS_UNAVAILABLE)?)
        .map_err(|_| ERROR_SCHEMA_DETAILS_UNAVAILABLE.into())
}

fn schema_failure(schema: &SchemaReport) -> String {
    let reason = match schema.status {
        SchemaStatus::UnsupportedNewer => "unsupported newer schema; use a binary supporting that version; downgrade execution is not supported",
        SchemaStatus::MissingHistory => "missing migration history; preserve the database and investigate against a trusted copy",
        SchemaStatus::ChecksumMismatch => "migration checksum mismatch; preserve the database and investigate against a trusted copy",
        SchemaStatus::InvalidHistory => "invalid migration history; preserve the database and investigate against a trusted copy",
        SchemaStatus::UpgradeRequired => "schema upgrade required; restart with a binary supporting the database version",
        SchemaStatus::Unknown => "unrecognized compatibility status; use a client supporting this daemon",
        SchemaStatus::Compatible => "inconsistent compatibility details; verified schema compatibility unavailable",
    };
    let applied = schema
        .applied_version
        .map_or("unavailable".into(), |v| v.to_string());
    format!(
        "{ERROR_SCHEMA_INCOMPATIBLE}: {reason}. Applied version: {applied}. Supported version: {}.",
        schema.supported_version
    )
}
