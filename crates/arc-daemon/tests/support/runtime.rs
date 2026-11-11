//! Daemon-only CLI/storage assertions over the shared owned native fixtures.
#[path = "native_fixture.rs"]
mod native_fixture;
pub use native_fixture::{assert_report, Fixture};
pub type OwnedChild = native_fixture::OwnedChild;
use std::{
    fs,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
};

impl Fixture {
    pub fn new() -> Self {
        Self::with_binary(PathBuf::from(env!("CARGO_BIN_EXE_arc-daemon")))
    }
    pub fn spawn(&self, profile: &str) -> OwnedChild {
        let mut command = self.command();
        command.env("ARC_IPC_PROFILE", profile);
        let mut child = OwnedChild::new(command.spawn().unwrap());
        self.wait_for(&mut child, "arc-daemon ready.");
        child
    }

    pub fn cli(&self, operation: &str) -> std::process::Output {
        let path = self.binary("arcctl");
        assert!(
            path.is_file(),
            "workspace CLI prerequisite: {}",
            path.display()
        );
        Command::new(path)
            .arg(operation)
            .env(arc_core::ARC_SOCKET_PATH, &self.socket)
            .env(arc_core::ARC_DB_PATH, &self.database)
            .output()
            .unwrap()
    }

    pub fn assert_cli(&self) {
        let status = self.cli("status");
        assert!(status.status.success(), "{status:?}");
        assert!(status.stderr.is_empty());
        let text = String::from_utf8(status.stdout).unwrap();
        assert!(text.contains("Sweeper off."), "{text}");
        assert!(text.contains(self.socket.to_str().unwrap()), "{text}");
        let doctor = self.cli("doctor");
        assert!(doctor.status.success(), "{doctor:?}");
        assert!(doctor.stderr.is_empty());
        let text = String::from_utf8(doctor.stdout).unwrap();
        assert_eq!(text, "ok. IPC owner-only. Tray not connected. Vendors not detected. Command classification not checked. Sweeper off. Schema compatible. Version 3. Migration history verified.\n");
        assert!(!text.contains('\u{1b}'));
    }

    pub fn report(&self) -> serde_json::Value {
        let bytes = fs::read(self.root.join("runtime.json")).unwrap();
        assert!(bytes.len() <= 4096);
        let report = serde_json::from_slice(&bytes).unwrap();
        assert_report(&report);
        assert!(!self.socket.exists());
        let database = arc_storage::open_and_migrate(&self.database).unwrap();
        let integrity: String = database
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");
        database.close().unwrap();
        report
    }
}

/// Preserve the existing helper API without unused re-export warnings in suites
/// that only use a subset of the shared native fixture ports.
pub fn connect(socket: &std::path::Path) -> UnixStream {
    native_fixture::connect(socket)
}

pub fn receipt(name: &str, value: &serde_json::Value) {
    eprintln!("bounded-runtime-{name}: {value}");
    if let Some(root) = std::env::var_os("ARC_TEST_EVIDENCE_DIR") {
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(format!("{name}.json")),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
}

pub fn assert_closed(mut stream: UnixStream, budget: Duration) {
    let start = Instant::now();
    // macOS may reject SO_RCVTIMEO after the peer has already disconnected.
    // Nonblocking reads also keep this observation on one absolute deadline.
    stream.set_nonblocking(true).unwrap();
    let mut byte = [0];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
                ) =>
            {
                break
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    start.elapsed() < budget,
                    "peer did not close within {budget:?}"
                );
                thread::sleep(Duration::from_millis(1));
            }
            value => panic!("expected closure: {value:?}"),
        }
    }
    assert!(start.elapsed() <= budget);
}

pub fn send(stream: &mut UnixStream, request: &arc_ipc::Request) {
    arc_ipc::write_message(stream, request).unwrap();
    stream.flush().unwrap();
}
