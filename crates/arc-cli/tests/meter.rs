//! Deterministic CLI peer fixtures; production native-daemon execution belongs
//! to the integration gate owner. These values are synthetic, not calibration.
use arc_ipc::{Request, Response};
use std::{
    fs, io,
    os::unix::{
        fs::{DirBuilderExt, PermissionsExt},
        net::UnixListener,
    },
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = arc_core::runtime_root().unwrap().join(format!(
            "m{:x}-{:x}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    fn cli(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_arcctl"))
            .args(args)
            .env(arc_core::ARC_SOCKET_PATH, self.0.join("ipc"))
            .output()
            .unwrap()
    }
    fn peer(&self, case: u8) -> std::process::Output {
        let listener = UnixListener::bind(self.0.join("ipc")).unwrap();
        fs::set_permissions(self.0.join("ipc"), fs::Permissions::from_mode(0o600)).unwrap();
        listener.set_nonblocking(true).unwrap();
        thread::scope(|scope| {
            let peer = scope.spawn(|| {
                let deadline = Instant::now() + Duration::from_secs(3);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "CLI never requested Meter");
                            thread::sleep(Duration::from_millis(1));
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let request: Request = arc_ipc::read_message(&mut stream).unwrap();
                assert_eq!(request.protocol_version, 1);
                assert_eq!(request.operation, "meter.snapshot");
                assert!(request.payload.is_null());
                let mut value: serde_json::Value = serde_json::from_str(include_str!(
                    "../../arc-core/tests/fixtures/meter-snapshot.json"
                ))
                .unwrap();
                let mut response = Response::ok(&request.request_id, value.clone());
                match case {
                    0 => {}
                    1 => {
                        response = Response::error(
                            &request.request_id,
                            arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                        )
                    }
                    2 => {
                        response = Response::error(
                            &request.request_id,
                            arc_ipc::meter::ERROR_METER_UNAVAILABLE,
                        )
                    }
                    3 => {
                        value["host"]["memory_available_bytes"]["reading"] =
                            serde_json::json!({"availability":"available","value":-1});
                        response.result = Some(value);
                    }
                    4 => response.request_id = "wrong-id".into(),
                    5 => response.protocol_version = 99,
                    6 => {
                        response.result =
                            Some(serde_json::json!({"untrusted":"peer-secret-canary"}))
                    }
                    _ => unreachable!(),
                }
                if let Some(error) = &mut response.error {
                    error.message = Some("peer-secret-canary".into());
                }
                arc_ipc::write_response(&mut stream, &response).unwrap();
            });
            let output = self.cli(&["meter", "snapshot", "--json"]);
            peer.join().unwrap();
            output
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn successful_json_snapshot_and_old_daemon_errors_are_typed_and_private() {
    for case in 0..=6 {
        let fixture = Fixture::new();
        let output = fixture.peer(case);
        if case == 0 {
            assert!(output.status.success(), "{output:?}");
            assert!(output.stderr.is_empty());
            let value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(arc_ipc::meter::decode_snapshot(&value).is_ok());
        } else {
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(
                stderr.starts_with(match case {
                    1 => "UNSUPPORTED_CAPABILITY",
                    2 => "METER_UNAVAILABLE",
                    5 => "PROTOCOL_INCOMPATIBLE",
                    _ => "IPC_ERROR",
                }),
                "{stderr}"
            );
            assert!(!stderr.contains("peer-secret-canary"));
        }
    }
}

#[test]
fn invalid_scope_is_rejected_without_connecting_and_help_is_available_headlessly() {
    let fixture = Fixture::new();
    for args in [
        &["meter", "snapshot"][..],
        &["meter", "snapshot", "--json", "--pid", "peer-secret-canary"],
        &["meter", "snapshot", "--json", "--path", "/secret"],
    ] {
        let output = fixture.cli(args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            b"INVALID_REQUEST\nUse arcctl meter snapshot --json or arcctl meter summary --job <JOB_ID> --json.\n"
        );
    }
    let help = fixture.cli(&["meter", "snapshot", "--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout).unwrap().contains("--json"));
    let missing = fixture.cli(&["meter", "snapshot", "--json"]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    assert!(String::from_utf8(missing.stderr)
        .unwrap()
        .starts_with("METER_UNAVAILABLE"));
}
