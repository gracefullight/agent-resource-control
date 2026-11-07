use super::*;
use std::{
    fs,
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    time::{SystemTime, UNIX_EPOCH},
};

fn no_canaries(root: &Path, needles: &[String]) {
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        let meta = fs::symlink_metadata(&path).unwrap();
        if meta.is_dir() {
            no_canaries(&path, needles);
        } else if meta.is_file() {
            let bytes = fs::read(&path).unwrap();
            for needle in needles {
                assert!(
                    !bytes.windows(needle.len()).any(|w| w == needle.as_bytes()),
                    "persistent canary detected"
                );
            }
        }
    }
}

#[test]
fn unix_endpoint_traps_substitutions_and_live_wal_never_receive_input() {
    exercise_privacy(false);
}

#[test]
fn required_loopback_endpoint_nonexecution_and_privacy() {
    exercise_privacy(true);
}

enum Monitor {
    Tcp(TcpListener),
    Unix(std::os::unix::net::UnixListener),
}
impl Monitor {
    fn assert_untouched(&self) {
        let result = match self {
            Self::Tcp(listener) => listener.accept().map(|_| ()),
            Self::Unix(listener) => listener.accept().map(|_| ()),
        };
        assert!(
            matches!(result, Err(e) if e.kind() == std::io::ErrorKind::WouldBlock),
            "endpoint contacted"
        );
    }
}

fn exercise_privacy(tcp: bool) {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let traps = fixture.root.join("traps");
    fs::create_dir(&traps).unwrap();
    for name in [
        "cargo",
        "gh",
        "ssh",
        "docker",
        "docker-compose",
        "sh",
        "bash",
        "zsh",
        "touch",
        "curl",
    ] {
        let path = traps.join(name);
        fs::write(
            &path,
            b"#!/bin/sh\nprintf 'started' > \"$ARC_TRAP_MARKER\"\nexit 99\n",
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let marker = fixture.root.join("execution-marker");
    let mut command = fixture.command();
    command.env("PATH", &traps).env("ARC_TRAP_MARKER", &marker);
    let mut child = OwnedChild::new(command.spawn().unwrap());
    fixture.wait_for(&mut child, "arc-daemon ready.");
    // Keep a reader open so any extant WAL is inspected before shutdown/checkpoint.
    let reader = rusqlite::Connection::open_with_flags(
        &fixture.database,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    reader
        .execute_batch("BEGIN; SELECT * FROM sqlite_master;")
        .unwrap();
    let (endpoint, address) = if tcp {
        let endpoint = TcpListener::bind("127.0.0.1:0")
            .expect("required loopback monitor prerequisite: TCP bind must be permitted");
        endpoint.set_nonblocking(true).unwrap();
        let address = format!("tcp://{}", endpoint.local_addr().unwrap());
        (Monitor::Tcp(endpoint), address)
    } else {
        let path = fixture.root.join("docker-monitor.sock");
        let endpoint = std::os::unix::net::UnixListener::bind(&path).unwrap();
        endpoint.set_nonblocking(true).unwrap();
        (
            Monitor::Unix(endpoint),
            format!("unix://{}", path.display()),
        )
    };
    let secret = format!(
        "arc-synthetic-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let substitution = format!("$(/usr/bin/touch {})", marker.display());
    let cases = [
        (format!("cargo test '{secret}'"), "CARGO_TEST"),
        (format!("gh workflow run '{secret}'"), "GH_WORKFLOW_RUN"),
        (format!("ssh build01 'echo {secret}'"), "SSH_REMOTE_COMMAND"),
        (
            format!("cargo test {substitution} {secret}"),
            "UNRESOLVED_EXPANSION",
        ),
        (
            format!("cargo test '{substitution}' '{secret}'"),
            "CARGO_TEST",
        ),
        (
            format!("sh -c 'cargo test {substitution} {secret}'"),
            "UNRESOLVED_EXPANSION",
        ),
        (
            format!("cargo test `/usr/bin/touch {}` {secret}", marker.display()),
            "UNRESOLVED_EXPANSION",
        ),
        (
            format!("docker -H {address} build '{secret}'"),
            "DOCKER_ENDPOINT_UNKNOWN",
        ),
        (
            format!("ssh -p 1234 127.0.0.1 'echo {secret}'"),
            "SSH_AMBIGUOUS",
        ),
        (
            format!("cargo test 'http://{address}/{secret}'"),
            "CARGO_TEST",
        ),
        (format!("./{secret}.sh"), "UNRESOLVED_SCRIPT"),
        (format!("cargo test '{secret}"), "UNSUPPORTED_SYNTAX"),
    ];
    let mut needles = vec![secret];
    needles.extend(cases.iter().map(|(text, _)| text.clone()));
    no_canaries(&fixture.root, &needles);
    let mut stream = fixture.connect();
    for (index, (text, reason)) in cases.iter().enumerate() {
        let response = exchange(&mut stream, json!({"command":text}));
        assert!(response.status == "ok", "privacy-{index}");
        assert!(
            response.result.as_ref().unwrap()["reason"] == *reason,
            "privacy-{index}"
        );
        let encoded = serde_json::to_vec(&response).unwrap();
        assert!(
            !encoded
                .windows(needles[0].len())
                .any(|w| w == needles[0].as_bytes()),
            "privacy-response-{index}"
        );
        for json in [false, true] {
            let mut command =
                Command::new(Path::new(env!("CARGO_BIN_EXE_arc-daemon")).with_file_name("arcctl"));
            command.arg("classify");
            if json {
                command.arg("--json");
            }
            let output = command
                .arg("--")
                .arg(text)
                .env(arc_core::ARC_SOCKET_PATH, &fixture.socket)
                .env("PATH", &traps)
                .env("ARC_TRAP_MARKER", &marker)
                .output()
                .unwrap();
            assert!(
                output.status.success() && output.stderr.is_empty(),
                "privacy-cli-{index}"
            );
            assert!(
                !output
                    .stdout
                    .windows(needles[0].len())
                    .any(|w| w == needles[0].as_bytes()),
                "privacy-output-{index}"
            );
        }
        assert!(!marker.exists(), "analyzed tool or substitution executed");
        endpoint.assert_untouched();
        no_canaries(&fixture.root, &needles); // SQLite main, live WAL/SHM, logs and artifacts
    }
    // Invalid input also never echoes its synthetic secret through protocol errors.
    let response = exchange(
        &mut stream,
        json!({"command":needles[0],"secret":needles[0]}),
    );
    assert_eq!(response.error.unwrap().code, arc_ipc::ERROR_INVALID_REQUEST);
    drop(stream);
    reader.execute_batch("ROLLBACK").unwrap();
    reader.close().unwrap();
    child.stop(libc::SIGTERM);
    no_canaries(&fixture.root, &needles);
    let report = fixture.report();
    assert_eq!(report["metrics"]["db_outstanding"]["high_water"], 0);
    assert!(!marker.exists());
    endpoint.assert_untouched();
    // Evidence retains identifiers/outcomes only, never command/secret canaries.
    runtime::receipt(
        "classification-privacy",
        &json!({"cases":cases.len(),"toolTrapsStarted":0,"substitutionMarkers":0,"endpointContacts":0,"canariesPersisted":0,"liveSqliteAndWalChecked":true}),
    );
}
