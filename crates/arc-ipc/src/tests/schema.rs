use super::*;
use crate::{SchemaReport, SchemaStatus};

fn report(version: u32) -> DoctorReport {
    DoctorReport {
        integrity_check: "ok".into(),
        schema_migrations: i64::from(version),
        schema: Some(SchemaReport {
            status: SchemaStatus::Compatible,
            applied_version: Some(version),
            supported_version: version,
            history_verified: true,
        }),
    }
}

// Baseline decoder/checks from bf28a35; no knowledge of the additive schema field.
#[derive(Deserialize)]
struct LegacyDoctor {
    integrity_check: String,
    schema_migrations: i64,
}
fn legacy_accepts(response: &Response) -> bool {
    response.status == STATUS_OK
        && response.protocol_version == 1
        && response.error.is_none()
        && response.result.clone().is_some_and(|value| {
            let decoded: LegacyDoctor = serde_json::from_value(value).unwrap();
            decoded.integrity_check == "ok" && decoded.schema_migrations == 1
        })
}

#[test]
fn legacy_request_and_decoder_accept_new_production_report_with_truthful_count() {
    let request = Request::decode(
        br#"{"protocol_version":1,"request_id":"old","operation":"diagnostics.doctor"}"#,
    )
    .unwrap();
    assert!(request.payload.is_null());
    let response = dispatch(&request, || Ok(report(1)));
    let mut bytes = Vec::new();
    write_response(&mut bytes, &response).unwrap();
    let decoded = read_response(&mut bytes.as_slice()).unwrap();
    assert!(legacy_accepts(&decoded));
    assert_eq!(decoded.request_id, "old");
    assert_eq!(decoded.result.unwrap()["schema"]["history_verified"], true);
}

#[test]
fn modern_version_two_succeeds_but_legacy_count_check_rejects_without_falsifying_count() {
    let response = dispatch(&Request::new("v2", OPERATION_DOCTOR), || Ok(report(2)));
    assert_eq!(response.status, STATUS_OK);
    assert_eq!(response.result.as_ref().unwrap()["schema_migrations"], 2);
    assert!(!legacy_accepts(&response));
}

#[test]
fn every_incompatibility_keeps_diagnostic_result_and_prevents_legacy_success() {
    for status in [
        SchemaStatus::UpgradeRequired,
        SchemaStatus::MissingHistory,
        SchemaStatus::ChecksumMismatch,
        SchemaStatus::InvalidHistory,
        SchemaStatus::UnsupportedNewer,
        SchemaStatus::Unknown,
    ] {
        let mut input = report(1);
        let schema = input.schema.as_mut().unwrap();
        schema.status = status;
        schema.history_verified = false;
        schema.applied_version = if status == SchemaStatus::UnsupportedNewer {
            Some(2)
        } else {
            None
        };
        let response = dispatch(&Request::new("bad", OPERATION_DOCTOR), || Ok(input.clone()));
        assert_eq!(response.status, STATUS_ERROR);
        assert_eq!(
            response.error.as_ref().unwrap().code,
            ERROR_SCHEMA_INCOMPATIBLE
        );
        assert_eq!(
            response.result.as_ref().unwrap()["schema"],
            json!(input.schema)
        );
        assert!(!legacy_accepts(&response));
    }
}

#[test]
fn present_but_inconsistent_details_never_fall_back_to_legacy_success() {
    {
        let mut input = report(1);
        input.schema.as_mut().unwrap().history_verified = false;
        assert_eq!(input.validation_error(), Some(ERROR_SCHEMA_INCOMPATIBLE));
    }
    for value in [
        json!({"status":"compatible","supported_version":1,"history_verified":true}),
        json!({"status":"compatible","applied_version":2,"supported_version":1,"history_verified":true}),
        json!({"status":"compatible","applied_version":0,"supported_version":0,"history_verified":true}),
        json!({"status":"future_status","applied_version":1,"supported_version":1,"history_verified":true}),
    ] {
        let mut input = report(1);
        input.schema = Some(serde_json::from_value(value).unwrap());
        assert_eq!(input.validation_error(), Some(ERROR_SCHEMA_INCOMPATIBLE));
    }
    let mut input = report(2);
    input.schema_migrations = 1;
    assert_eq!(input.validation_error(), Some(ERROR_SCHEMA_INCOMPATIBLE));
}

#[test]
fn legacy_absence_and_null_decode_but_missing_present_fields_are_invalid() {
    for schema in [None, Some(Value::Null)] {
        let mut input = json!({"integrity_check":"ok","schema_migrations":1});
        if let Some(schema) = schema {
            input["schema"] = schema;
        }
        let decoded: DoctorReport = serde_json::from_value(input).unwrap();
        assert_eq!(decoded.schema, None);
        assert_eq!(decoded.validation_error(), None);
        assert!(serde_json::to_value(decoded)
            .unwrap()
            .get("schema")
            .is_none());
    }
    for schema in [
        json!({}),
        json!({"status":"compatible","supported_version":1}),
        json!({"supported_version":1,"history_verified":true}),
    ] {
        assert!(serde_json::from_value::<DoctorReport>(json!({
            "integrity_check":"ok","schema_migrations":1,"schema":schema
        }))
        .is_err());
    }
}

#[test]
fn protocol_mismatch_precedes_doctor_and_integrity_failure_precedes_schema() {
    let mut request = Request::new("old", OPERATION_DOCTOR);
    request.protocol_version = 99;
    assert_eq!(
        dispatch(&request, || unreachable!()).error.unwrap().code,
        ERROR_PROTOCOL_INCOMPATIBLE
    );
    request.protocol_version = 1;
    let mut input = report(2);
    input.integrity_check = "failed".into();
    assert_eq!(
        dispatch(&request, || Ok(input)).error.unwrap().code,
        ERROR_INTEGRITY_FAILED
    );
}
