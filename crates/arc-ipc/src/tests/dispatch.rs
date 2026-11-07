use super::*;

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
fn legacy_doctor_requires_ok_integrity_and_one_migration() {
    let request = Request::new("req", OPERATION_DOCTOR);
    let response = dispatch(&request, || {
        Ok(DoctorReport {
            integrity_check: "ok".to_owned(),
            schema_migrations: 1,
            schema: None,
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
            schema: None,
        })
    });
    assert_eq!(
        response.error.map(|error| error.code),
        Some(ERROR_SCHEMA_DETAILS_UNAVAILABLE.to_owned())
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
